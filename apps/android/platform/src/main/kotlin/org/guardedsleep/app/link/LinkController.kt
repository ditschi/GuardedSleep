package org.guardedsleep.app.link

import java.security.SecureRandom
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.guardedsleep.app.alerts.AlertSink
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.config.AppDefaults
import org.guardedsleep.app.config.DebugOptions
import org.guardedsleep.app.config.PairingPolicy
import org.guardedsleep.app.config.RelayStun
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.IceConfig
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnServerInput
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.readiness.ReadinessChecker
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.silence.SilenceGateway
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.PairingError
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.TurnSaveResult
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.LinkConfig
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkInput
import uniffi.guardedsleep_core.LinkState
import uniffi.guardedsleep_core.LinkStatus
import uniffi.guardedsleep_core.Role as CoreRole
import uniffi.guardedsleep_core.defaultLinkConfig

/**
 * The platform layer's single entry point for the UI. Owns at most one [LinkRunner] (one
 * [uniffi .guardedsleep_core.CoreLink]) per monitoring or pairing run, keeps the persisted choices,
 * and maps the core's status and events onto the UI models. Decisions about the link itself stay in
 * the core.
 */
class LinkController(
    private val settingsRepo: SettingsRepository,
    private val pairingStore: PairingStore,
    private val turnStore: TurnStore,
    private val signaling: SignalingFactory,
    private val mediaFactory: (Role, IceConfig) -> MediaEngine,
    private val silence: SilenceGateway,
    private val alerts: AlertSink,
    private val readiness: ReadinessProvider,
    private val host: MonitorHost,
    private val network: NetworkWatcher,
    private val clock: Clock = SystemClockSource,
    private val newExecutor: () -> ScheduledExecutorService = {
        Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "gs-core") }
    },
    private val random: SecureRandom = SecureRandom(),
) : AppController {
    private val _app = MutableStateFlow(AppState())
    // The sealed TURN entry; the disk copy is written on [turnIo] so the caller never blocks.
    @Volatile private var turn: TurnCredentials? = turnStore.load()
    private val turnIo = Executors.newSingleThreadExecutor { r ->
        Thread(r, "gs-turn-io").apply { isDaemon = true }
    }
    private val _settings = MutableStateFlow(withTurn(settingsRepo.current.settings, turn))
    private val _pairing = MutableStateFlow<PairingState?>(null)

    override val app: StateFlow<AppState> = _app
    override val settings: StateFlow<SettingsState> = _settings
    override val pairing: StateFlow<PairingState?> = _pairing
    override val parent: StateFlow<ParentMonitorState>
        get() = parentModel.state

    override val baby: StateFlow<BabyMonitorState>
        get() = babyModel.state

    private val parentEffects =
        object : ParentEffects {
            override fun muteRemote(muted: Boolean) {
                run?.runner?.media { it.setPlaybackMuted(muted) }
            }

            override fun listenStopped() {
                run?.runner?.input(LinkInput.SetListen(false))
            }

            override fun testAlertArrived(wallMs: Long) {
                settingsRepo.update { it.copy(lastTestWallMs = wallMs) }
            }

            override fun snoozeEnded() {
                settingsRepo.update { it.copy(snoozeUntilWallMs = null) }
            }
        }

    private val parentModel =
        ParentModel(clock, alerts, parentEffects) { _settings.value.threshold.toDouble() }
    private val babyModel = BabyModel(clock)

    private class PendingPairing(
        val psk: String,
        val urls: List<String>,
        val turn: TurnCredentials?,
        val startedAt: Long,
        val expiresAtElapsedMs: Long?,
        var unreachableShown: Boolean = false,
    )

    private inner class Run(
        val id: Int,
        val role: Role,
        var pending: PendingPairing?,
        var monitoring: Boolean,
    ) {
        lateinit var runner: LinkRunner
        var autoListenDone = false
    }

    @Volatile private var lastReadinessAt = clock.elapsedMs()
    @Volatile private var run: Run? = null
    private var runCounter = 0

    init {
        refreshApp()
        babyModel.setSettings(_settings.value.threshold, _settings.value.holdSeconds)
        babyModel.setSilence(null, silence.hasAccess())
        babyModel.setSilenceRestoredNotice(silence.restoreLeftover())
        parentModel.restoreSnooze(settingsRepo.current.snoozeUntilWallMs)
    }

    // ---- app ---------------------------------------------------------------------------------

    override fun chooseRole(role: Role) {
        val persisted = settingsRepo.current
        if (persisted.role != null && persisted.role != role) forgetPairing()
        settingsRepo.update { it.copy(role = role) }
        refreshApp()
    }

    override fun completeOnboarding() {
        settingsRepo.update { it.copy(onboardingDone = true) }
        refreshApp()
    }

    override fun updateSettings(transform: (SettingsState) -> SettingsState) {
        val before = _settings.value
        // TURN fields live in the sealed store and change only through [setTurnServer].
        val after =
            transform(before)
                .copy(
                    turnUrl = before.turnUrl,
                    turnUsername = before.turnUsername,
                    turnPasswordSaved = before.turnPasswordSaved,
                )
        if (after == before) return
        settingsRepo.update { it.copy(settings = after) }
        _settings.value = after
        babyModel.setSettings(after.threshold, after.holdSeconds)
        val r = run
        if (r != null) {
            if (
                r.role == Role.BABY &&
                    (after.threshold != before.threshold || after.holdSeconds != before.holdSeconds)
            ) {
                r.runner.input(
                    LinkInput.SetNoiseGate(
                        after.threshold.toDouble(),
                        (after.holdSeconds * 1000L).toULong(),
                    )
                )
            }
            if (after.alertGainPct != before.alertGainPct)
                r.runner.media { it.setPlaybackGainPct(after.alertGainPct) }
            if (after.alertOutput != before.alertOutput) {
                r.runner.media { it.setSpeakerOnly(after.alertOutput == AlertOutput.SPEAKER_ONLY) }
            }
            if (
                r.role == Role.BABY &&
                    r.monitoring &&
                    after.silenceWhileMonitoring != before.silenceWhileMonitoring
            ) {
                applySilence(after.silenceWhileMonitoring)
            }
        }
    }

    @Synchronized
    override fun setTurnServer(url: String, username: String, password: String): TurnSaveResult {
        val u = url.trim()
        if (u.isEmpty()) {
            clearTurnServer()
            return TurnSaveResult.CLEARED
        }
        val n = username.trim()
        val saved = turn
        val keep = password.isEmpty() && saved != null && saved.url == u && saved.username == n
        val effective = if (keep) saved.credential else password
        return when (val r = TurnServerInput.validate(u, n, effective)) {
            TurnServerInput.Result.InvalidAddress -> TurnSaveResult.INVALID_ADDRESS
            TurnServerInput.Result.InvalidCredentials -> TurnSaveResult.INVALID_CREDENTIALS
            is TurnServerInput.Result.Ok -> {
                val next = TurnCredentials(r.url, r.username, r.password)
                turn = next
                _settings.value = withTurn(_settings.value, next)
                turnIo.execute { runCatching { turnStore.save(next) } }
                TurnSaveResult.SAVED
            }
        }
    }

    @Synchronized
    override fun clearTurnServer() {
        turn = null
        _settings.value = withTurn(_settings.value, null)
        turnIo.execute { runCatching { turnStore.clear() } }
    }

    override fun forgetPairing() {
        stopMonitoring()
        cancelPairing()
        silence.deactivate()
        babyModel.setSilence(null, silence.hasAccess())
        pairingStore.clear()
        settingsRepo.update { it.copy(peerName = null, snoozeUntilWallMs = null) }
        parentModel.cancelSnooze()
        refreshApp()
    }

    /**
     * MainActivity start/resume: resume a monitoring flag that outlived the process and re-check
     * readiness.
     */
    fun onForeground() {
        if (
            settingsRepo.current.monitoringActive &&
                (run == null || fgs == Fgs.LOST) &&
                pairingStore.exists()
        )
            startHost()
        refreshReadiness()
    }

    fun refreshReadiness() {
        if (settingsRepo.current.role == Role.PARENT)
            parentModel.setReadiness(readiness.check(), readiness.ownBatteryPct())
    }

    private enum class Fgs {
        UNKNOWN,
        RUNNING,
        LOST,
    }

    @Volatile private var fgs = Fgs.UNKNOWN

    /**
     * Asks for the foreground service; a refusal is visible (red state, parent alarm), never
     * swallowed.
     */
    private fun startHost() {
        fgs = Fgs.UNKNOWN
        try {
            host.start()
        } catch (e: Exception) {
            PocLog.event("failed", "reason" to "fgs_start_${e.javaClass.simpleName}")
            onServiceRunning(false)
        }
    }

    /** MonitorService reports whether it is in the foreground. */
    fun onServiceRunning(running: Boolean) {
        // A deliberate stop also ends the service; only an unexpected end counts.
        if (!running && !settingsRepo.current.monitoringActive) return
        fgs = if (running) Fgs.RUNNING else Fgs.LOST
        refreshApp()
        publishFgsAlert()
    }

    private fun publishFgsAlert() {
        val lost = fgs == Fgs.LOST && settingsRepo.current.monitoringActive
        // Only the parent sounds an alarm; the baby unit never makes a sound.
        if (settingsRepo.current.role == Role.PARENT) alerts.backgroundProtection(lost)
    }

    private fun refreshApp() {
        val p = settingsRepo.current
        _app.value =
            AppState(
                role = p.role,
                onboardingDone = p.onboardingDone,
                paired = pairingStore.exists(),
                peerName = p.peerName,
                monitoring = p.monitoringActive,
                backgroundProtection = !(p.monitoringActive && fgs == Fgs.LOST),
            )
    }

    // ---- pairing -----------------------------------------------------------------------------

    override fun startPairing() {
        if (settingsRepo.current.role != Role.BABY) return
        cancelPairing()
        stopRun(userStop = true)
        val urls = effectiveUrls(null)
        if (urls.isEmpty()) {
            _pairing.value = PairingState(PairingPhase.FAILED, error = PairingError.UNREACHABLE)
            return
        }
        val psk = ByteArray(PairingPolicy.PSK_BYTES).also(random::nextBytes)
        val now = clock.elapsedMs()
        val payload = PairingPayload.create(psk, urls, clock.wallMs())
        val pending =
            PendingPairing(payload.pskB64url, urls, null, now, now + PairingPolicy.QR_VALIDITY_MS)
        _pairing.value =
            PairingState(
                PairingPhase.SHOWING_CODE,
                codePayload = payload.toJson(),
                expiresAtElapsedMs = pending.expiresAtElapsedMs,
            )
        startRun(Role.BABY, pending, pairedBefore = false, monitoring = false, isPending = true)
    }

    override fun refreshPairingCode() = startPairing()

    override fun cancelPairing() {
        val r = run
        if (r?.pending != null) stopRun(userStop = true)
        _pairing.value = null
    }

    override fun submitPairingCode(payload: String) {
        if (settingsRepo.current.role != Role.PARENT) return
        when (val parsed = PairingPayload.parse(payload, clock.wallMs())) {
            is PairingPayload.Companion.Result.Rejected ->
                _pairing.value = PairingState(PairingPhase.FAILED, error = parsed.error)
            is PairingPayload.Companion.Result.Ok -> {
                stopRun(userStop = true)
                val p = parsed.payload
                val now = clock.elapsedMs()
                _pairing.value = PairingState(PairingPhase.CONNECTING)
                // Persisted only after the first verification, so a wrong code never leaves a
                // pairing.
                startRun(
                    Role.PARENT,
                    PendingPairing(p.pskB64url, p.signalingUrls, null, now, null),
                    pairedBefore = false,
                    monitoring = false,
                    isPending = true,
                )
            }
        }
    }

    // ---- monitoring --------------------------------------------------------------------------

    override fun startMonitoring() {
        settingsRepo.update { it.copy(monitoringActive = true) }
        refreshApp()
        startHost()
    }

    /** Called by the service once it is in the foreground: starts (or adopts) the run. */
    fun ensureRun() {
        val existing = run
        if (existing != null) {
            promote(existing)
            return
        }
        val role = settingsRepo.current.role ?: return
        val stored = pairingStore.load() ?: return
        resumeRun(role, stored)
    }

    private fun promote(r: Run) {
        if (r.monitoring) return
        r.monitoring = true
        if (r.role == Role.BABY) applySilence(_settings.value.silenceWhileMonitoring)
    }

    override fun stopMonitoring() {
        settingsRepo.update { it.copy(monitoringActive = false) }
        stopRun(userStop = true)
        fgs = Fgs.UNKNOWN
        refreshApp()
        publishFgsAlert()
        host.stop()
    }

    private fun resumeRun(role: Role, stored: StoredPairing) =
        startRun(
            role,
            PendingPairing(
                stored.pskB64url,
                stored.signalingUrls,
                stored.turn,
                clock.elapsedMs(),
                null,
            ),
            stored.pairedBefore,
            monitoring = true,
            isPending = false,
        )

    private fun startRun(
        role: Role,
        pending: PendingPairing,
        pairedBefore: Boolean,
        monitoring: Boolean,
        isPending: Boolean,
    ) {
        stopRun(userStop = false)
        val s = _settings.value
        val coreRole = if (role == Role.BABY) CoreRole.BABY else CoreRole.PARENT
        val config = buildConfig(coreRole, pending.psk, pairedBefore, s)
        val r = Run(++runCounter, role, if (isPending) pending else null, monitoring)
        val signalingUrls = effectiveUrls(pending.urls)
        val ice =
            buildIceConfig(
                RelayStun.urls(signalingUrls),
                pending.turn,
                turn,
                DebugOptions.relayOnly,
            )
        val runner =
            LinkRunner(
                config = config,
                signalingUrls = signalingUrls,
                signaling = signaling,
                mediaFactory = { mediaFactory(role, ice) },
                clock = clock,
                executor = newExecutor(),
                listener = RunListener(r),
                deviceStatus = if (role == Role.BABY) ::babyDeviceStatus else null,
                applyQuiet = if (role == Role.BABY) ::applyQuietNow else null,
                entropy = random.nextLong(),
            )
        r.runner = runner
        synchronized(this) { run = r }
        parentModel.reset()
        babyModel.reset()
        babyModel.setSilence(null, silence.hasAccess())
        network.start { runner.input(LinkInput.NetworkAvailable) }
        runner.start()
        publishFgsAlert()
        if (role == Role.PARENT) {
            runner.media {
                it.setPlaybackGainPct(s.alertGainPct)
                it.setSpeakerOnly(s.alertOutput == AlertOutput.SPEAKER_ONLY)
            }
            parentModel.snoozeUntilElapsedMs()?.let {
                runner.input(LinkInput.SetSnooze(it.toULong()))
            }
            refreshReadiness()
        } else if (monitoring) {
            applySilence(s.silenceWhileMonitoring)
        }
    }

    private fun stopRun(userStop: Boolean) {
        val r = synchronized(this) { run.also { run = null } } ?: return
        network.stop()
        if (userStop) r.runner.stop() else r.runner.abort()
        parentModel.reset()
        babyModel.reset()
        if (r.role == Role.BABY) {
            silence.deactivate()
            babyModel.setSilence(null, silence.hasAccess())
        }
        alerts.clear()
    }

    private fun applySilence(enabled: Boolean) {
        if (!enabled) {
            silence.deactivate()
            babyModel.setSilence(null, silence.hasAccess())
            return
        }
        babyModel.setSilence(silence.activate(), silence.hasAccess())
    }

    private fun babyDeviceStatus(): DeviceStatus = silence.readDeviceStatus()

    private fun applyQuietNow(): DeviceStatus {
        babyModel.setSilence(silence.activate(), silence.hasAccess())
        return silence.readDeviceStatus()
    }

    private fun buildConfig(
        role: CoreRole,
        psk: String,
        pairedBefore: Boolean,
        s: SettingsState,
    ): LinkConfig {
        val base = defaultLinkConfig(role, psk)
        val heartbeat =
            (DebugOptions.heartbeatMs ?: (s.heartbeatSeconds * 1000L)).coerceIn(5_000L, 15_000L)
        val liveness = 2 * heartbeat + 5_000
        var alarmAfter = DebugOptions.alarmAfterMs ?: AppDefaults.ALARM_AFTER_MS
        // The core rejects a link whose loss detection is not at least 20 s ahead of the alarm.
        if (alarmAfter < liveness + 20_000) alarmAfter = liveness + 20_000
        return base.copy(
            pairedBefore = pairedBefore,
            heartbeatIntervalMs = heartbeat.toULong(),
            alarmAfterMs = alarmAfter.toULong(),
            noiseThreshold = s.threshold.toDouble(),
            noiseHoldMs = (DebugOptions.noiseHoldMs ?: (s.holdSeconds * 1000L)).toULong(),
        )
    }

    /** Advanced list overrides the pairing's list; otherwise the QR's list (or the defaults). */
    private fun effectiveUrls(pairingUrls: List<String>?): List<String> {
        val s = _settings.value
        return when {
            s.advancedEnabled && s.signalingUrls.isNotEmpty() -> s.signalingUrls
            !pairingUrls.isNullOrEmpty() -> pairingUrls
            else -> s.signalingUrls
        }
    }

    // ---- parent intents ----------------------------------------------------------------------

    override fun setListen(mode: ListenMode) {
        val r = run ?: return
        if (parentModel.setListen(mode, clock.elapsedMs())) {
            r.runner.input(LinkInput.SetListen(mode != ListenMode.OFF))
        }
    }

    override fun requestTestAlert() {
        val r = run
        if (r != null && parentModel.state.value.link.phase == LinkPhase.CONNECTED) {
            r.runner.input(LinkInput.RequestProbe)
        } else {
            // Tested this phone only: the chain runs locally with the baby phone not involved.
            alerts.noiseAlert(NoiseKind.NEW, probe = true, snoozed = false)
            settingsRepo.update { it.copy(lastTestWallMs = clock.wallMs()) }
        }
    }

    override fun snooze(minutes: Int) {
        val wallUntil = parentModel.snooze(minutes)
        settingsRepo.update { it.copy(snoozeUntilWallMs = wallUntil) }
        parentModel.snoozeUntilElapsedMs()?.let {
            run?.runner?.input(LinkInput.SetSnooze(it.toULong()))
        }
    }

    override fun cancelSnooze() {
        parentModel.cancelSnooze()
        settingsRepo.update { it.copy(snoozeUntilWallMs = null) }
        run?.runner?.input(LinkInput.SetSnooze(null))
    }

    override fun acknowledgeAlarm() {
        run?.runner?.input(LinkInput.AcknowledgeAlarm)
    }

    override fun stopAlertSound() = parentModel.stopAlertSound()

    override fun requestSilenceBaby() {
        val r = run ?: return
        parentModel.requestSilence(clock.elapsedMs())
        r.runner.input(LinkInput.RequestQuiet)
    }

    override fun tryReconnectNow() {
        run?.runner?.input(LinkInput.NetworkAvailable)
    }

    override fun dismissMissedEvents() = parentModel.dismissMissed()

    // ---- baby intents ------------------------------------------------------------------------

    override fun setKioskLocked(locked: Boolean) = babyModel.setKioskLocked(locked)

    override fun dismissSilenceRestoredNotice() = babyModel.setSilenceRestoredNotice(false)

    // ---- core thread -------------------------------------------------------------------------

    private inner class RunListener(private val r: Run) : LinkListener {
        private fun current() = run === r

        override fun onStatus(status: LinkStatus, nowMs: Long) {
            if (!current()) return
            if (r.role == Role.PARENT) parentModel.onStatus(status, nowMs)
            else babyModel.onStatus(status, nowMs)
            onPairingStatus(status)
            if (
                status.state == LinkState.CONNECTED &&
                    !r.autoListenDone &&
                    r.role == Role.PARENT &&
                    DebugOptions.autoListen
            ) {
                r.autoListenDone = true
                parentModel.setListen(ListenMode.LOCKED, nowMs)
                r.runner.input(LinkInput.SetListen(true))
            }
        }

        override fun onEvent(event: LinkEvent, nowMs: Long) {
            if (!current()) return
            if (r.role == Role.PARENT) parentModel.onEvent(event, nowMs)
            else babyModel.onEvent(event, nowMs)
        }

        override fun onTick(nowMs: Long) {
            if (!current()) return
            if (r.role == Role.PARENT) {
                parentModel.onTick(nowMs)
                if (nowMs - lastReadinessAt >= ReadinessChecker.RECHECK_MS) {
                    lastReadinessAt = nowMs
                    refreshReadiness()
                }
            }
            checkPairingTimers(nowMs)
        }

        override fun onMicLevel(rms: Double, nowMs: Long) {
            if (current()) babyModel.onMicLevel(rms, nowMs)
        }

        override fun onRemoteLevel(rms: Double, nowMs: Long) {
            if (current()) parentModel.onRemoteLevel(rms, nowMs)
        }

        override fun onInboundStats(stats: InboundAudioStats, nowMs: Long) {
            if (!current()) return
            PocLog.event("stats", "packets_received" to stats.packetsReceived)
            parentModel.onInboundStats(stats, nowMs)
        }

        override fun onTornDown() = Unit

        private fun onPairingStatus(status: LinkStatus) {
            val pending = r.pending ?: return
            when (status.state) {
                LinkState.CONNECTED -> {
                    pairingStore.save(StoredPairing(pending.psk, pending.urls, true, pending.turn))
                    r.pending = null
                    PocLog.event("pairing_saved")
                    // The pairing link becomes the monitoring run; the other phone never sees a
                    // gap.
                    settingsRepo.update { it.copy(monitoringActive = true) }
                    refreshApp()
                    startHost()
                    // Last, so a UI that reacts to PAIRED already sees a consistent monitoring
                    // state.
                    _pairing.value = PairingState(PairingPhase.PAIRED)
                }
                LinkState.FAILED_NEEDS_USER -> {
                    val error =
                        if (status.key == "pairing_key_mismatch") PairingError.KEY_MISMATCH
                        else PairingError.UNREACHABLE
                    _pairing.value = PairingState(PairingPhase.FAILED, error = error)
                    PocLog.event("failed", "reason" to (status.key ?: "failed_needs_user"))
                    endPendingRun()
                }
                else -> Unit
            }
        }

        private fun checkPairingTimers(now: Long) {
            val pending = r.pending ?: return
            val expiry = pending.expiresAtElapsedMs
            if (r.role == Role.BABY && expiry != null && now >= expiry) {
                // The code is dead: close the room so nobody can still use it.
                _pairing.value = PairingState(PairingPhase.EXPIRED)
                endPendingRun()
                return
            }
            if (
                r.role == Role.PARENT &&
                    !pending.unreachableShown &&
                    now - pending.startedAt >= UNREACHABLE_AFTER_MS
            ) {
                pending.unreachableShown = true
                _pairing.value =
                    PairingState(PairingPhase.CONNECTING, error = PairingError.UNREACHABLE)
            }
        }

        private fun endPendingRun() {
            val owned =
                synchronized(this@LinkController) { (run === r).also { if (it) run = null } }
            if (!owned) return
            network.stop()
            r.runner.stop()
            parentModel.reset()
            babyModel.reset()
        }
    }

    private companion object {
        const val UNREACHABLE_AFTER_MS = 20_000L
    }
}

private fun withTurn(s: SettingsState, turn: TurnCredentials?): SettingsState =
    s.copy(
        turnUrl = turn?.url,
        turnUsername = turn?.username?.takeIf { it.isNotEmpty() },
        turnPasswordSaved = turn?.credential?.isNotEmpty() == true,
    )

/** The debug launch entry (`pending`) wins over the one saved in the settings. */
internal fun buildIceConfig(
    stunUrls: List<String>,
    pending: TurnCredentials?,
    stored: TurnCredentials?,
    relayOnly: Boolean,
): IceConfig {
    val t = pending ?: stored
    return IceConfig(
        stunUrls = stunUrls,
        turnUrl = t?.url,
        turnUser = t?.username?.takeIf { it.isNotEmpty() },
        turnCredential = t?.credential?.takeIf { it.isNotEmpty() },
        relayOnly = relayOnly,
    )
}
