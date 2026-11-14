package org.guardedsleep.app.link

import java.security.SecureRandom
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.guardedsleep.app.alerts.AlertSink
import org.guardedsleep.app.alerts.EscalationPolicy
import org.guardedsleep.app.alerts.LinkSnapshot
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.config.AppDefaults
import org.guardedsleep.app.config.DebugOptions
import org.guardedsleep.app.config.PairingPolicy
import org.guardedsleep.app.config.PublicLists
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.IceConfig
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.pairing.ConnectionName
import org.guardedsleep.app.pairing.PairingLoad
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.StoredMailbox
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnServerInput
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.pairing.signalingUrls
import org.guardedsleep.app.pairing.storedMailboxesFor
import org.guardedsleep.app.pairing.toStored
import org.guardedsleep.app.platform.BuildConfig
import org.guardedsleep.app.readiness.ReadinessChecker
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.silence.SilenceGateway
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.PairingError
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.TalkPermission
import org.guardedsleep.app.ui.model.TurnSaveResult
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.InviteMailbox
import uniffi.guardedsleep_core.LinkConfig
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkInput
import uniffi.guardedsleep_core.LinkState
import uniffi.guardedsleep_core.LinkStatus
import uniffi.guardedsleep_core.ParentSecret
import uniffi.guardedsleep_core.Role as CoreRole
import uniffi.guardedsleep_core.defaultLinkConfig
import uniffi.guardedsleep_core.filterOnlyLocal

/**
 * The platform layer's single entry point for the UI. Owns at most one [LinkRunner] (one
 * [uniffi .guardedsleep_core.CoreLink]) per monitoring or pairing run, keeps the persisted choices,
 * and maps the core's status and events onto the UI models. Decisions about the link itself stay in
 * the core.
 */
class LinkController(
    private val settingsRepo: SettingsRepository,
    private val pairings: RolePairings,
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
    /**
     * Parent: the name this device shows on the baby device ("Connected to {name}"), sent over the
     * verified DataChannel only. The wiring passes a model-based display name; never an identifier
     * or a fingerprint.
     */
    private val deviceName: () -> String = { "" },
    /** Parent: microphone permission and unlocked device, asked before every `set_talk`. */
    private val talkGate: TalkGate = TalkGate.Open,
    private val chimePreview: TalkChimePreview = TalkChimePreview {},
) : AppController {
    private val _app = MutableStateFlow(AppState())

    /**
     * The pairing this session uses: the user's choice while it still exists, else the default
     * pairing, else slot 1 (nothing saved yet). The baby unit runs only one link in this version,
     * so with several saved parent pairings it uses the default one.
     */
    private fun activeId(role: Role): String {
        val chosen = settingsRepo.current.pairingId
        return chosen?.takeIf { it in pairings.savedIds(role) }
            ?: pairings.defaultId(role)
            ?: pairings.ids(role).first()
    }

    /** The slot a new pairing is saved to: the parent takes a free one, the baby replaces. */
    private fun newPairingId(role: Role): String? =
        if (role == Role.PARENT) pairings.freeId(role) else activeId(role)

    /** The store of the pairing this session uses; null on the role screen. */
    private val pairingStoreOrNull: PairingStore?
        get() = settingsRepo.current.role?.let { pairings.store(it, activeId(it)) }

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

    private fun isDiscreet() = _settings.value.alertStyle == AlertStyle.DISCREET

    /**
     * A parent alert sound ends a running talk first (`alert_preempts_talk`): the sink asks the
     * core before it sounds anything.
     */
    private val preemptingAlerts: AlertSink =
        object : AlertSink by alerts {
            private fun preempt() {
                run?.takeIf { it.role == Role.PARENT && parentModel.talkActive() }
                    ?.runner
                    ?.input(LinkInput.AlertPreemptsTalk)
            }

            override fun linkState(snapshot: LinkSnapshot) {
                if (snapshot.level >= 2) preempt()
                alerts.linkState(snapshot)
            }

            override fun linkLost(downMs: Long) {
                preempt()
                alerts.linkLost(downMs)
            }

            override fun noiseAlert(
                kind: NoiseKind,
                probe: Boolean,
                snoozed: Boolean,
                discreet: Boolean,
            ) {
                preempt()
                alerts.noiseAlert(kind, probe, snoozed, discreet)
            }
        }

    private val parentModel =
        ParentModel(
            clock,
            preemptingAlerts,
            parentEffects,
            discreet = ::isDiscreet,
        ) {
            _settings.value.threshold.toDouble()
        }
    private val babyModel = BabyModel(clock)

    private class PendingPairing(
        val psk: String,
        val turn: TurnCredentials?,
        val startedAt: Long,
        val expiresAtElapsedMs: Long?,
        var unreachableShown: Boolean = false,
        /** The parent secret of a stored pairing (a pending first pairing has none yet). */
        val parentSecret: ByteArray? = null,
        /**
         * The pairing's mailbox set: the invite's (baby: the one it built) or the stored record's.
         * Empty opens no relay mailbox; there is no fallback to settings or defaults.
         */
        val mailboxes: List<StoredMailbox> = emptyList(),
        /**
         * Secret and mailbox set the core reported before the first verification saved the record
         * (the events can come before or after the `connected` status).
         */
        var pinnedSecret: ByteArray? = null,
        var receivedMailboxes: List<StoredMailbox>? = null,
        /**
         * The first-pairing window (Unix seconds): the expiry of the invite, then the last
         * `window_until_s` of `ParentPinned`. Null once the baby acknowledged the secret.
         */
        var pinUntilS: Long? = null,
        /** The peer's device name once known: the default connection name. Never logged. */
        var peerDeviceName: String? = null,
    )

    private inner class Run(
        val id: Int,
        val role: Role,
        /** The pairing slot this run reads and writes. */
        val slot: String,
        var pending: PendingPairing?,
        var monitoring: Boolean,
    ) {
        val store: PairingStore
            get() = pairings.store(role, slot)

        lateinit var runner: LinkRunner
        var autoListenDone = false
    }

    @Volatile private var lastReadinessAt = clock.elapsedMs()
    @Volatile private var run: Run? = null
    private var runCounter = 0

    init {
        // The pre-roles single pairing belongs to the role that install had chosen.
        pairings.migrateLegacy(settingsRepo.current.role)
        // Cold start: a running monitoring keeps its role (the service resumes it); otherwise the
        // start mode decides, and null shows the role screen.
        if (!settingsRepo.current.monitoringActive) {
            // The connection choice is per session: a cold start preselects the default again.
            settingsRepo.update {
                it.copy(
                    role = it.defaultRole,
                    pairingId = null,
                    parked = it.parked.mapValues { (_, l) -> l.copy(pairingId = null) },
                )
            }
        }
        refreshApp()
        babyModel.setSettings(_settings.value.threshold, _settings.value.holdSeconds)
        babyModel.setSilence(null, silence.hasAccess())
        babyModel.setSilenceRestoredNotice(silence.restoreLeftover())
        parentModel.restoreSnooze(settingsRepo.current.snoozeUntilWallMs)
    }

    // ---- app ---------------------------------------------------------------------------------

    override fun chooseRole(role: Role, makeDefault: Boolean) {
        // Both pairings are kept: a role change never forgets one.
        settingsRepo.update {
            it.copy(role = role, defaultRole = if (makeDefault) role else it.defaultRole)
        }
        parentModel.restoreSnooze(settingsRepo.current.snoozeUntilWallMs)
        refreshApp()
        // The first role choice of this app start may be the "app opens in parent mode".
        if (role == Role.PARENT) autoStartOnce() else autoStartPending = false
    }

    override fun switchMode() {
        autoStartPending = false
        if (settingsRepo.current.monitoringActive) return
        cancelPairing()
        stopRun(userStop = true)
        settingsRepo.update { it.copy(role = null) }
        parentModel.cancelSnooze()
        refreshApp()
    }

    override fun setDefaultRole(role: Role?) {
        settingsRepo.update { it.copy(defaultRole = role) }
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
        if (after.alertStyle != before.alertStyle) parentModel.onAlertStyleChanged()
        if (
            after.alertStyle != before.alertStyle ||
                after.discreetEscalation != before.discreetEscalation
        ) {
            run?.takeIf { it.role == Role.PARENT }?.let { sendEscalation(it.runner, after) }
        }
        val r = run
        if (r != null && after.onlyLocal != before.onlyLocal) restartForOnlyLocal(r)
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
            if (
                r.role == Role.BABY &&
                    (after.talkBackEnabled != before.talkBackEnabled ||
                        after.talkVolume != before.talkVolume ||
                        after.talkChime != before.talkChime)
            ) {
                sendTalkBack(r.runner, after)
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

    /**
     * The switch changes the mailbox set the core is given, so a running run starts again from the
     * stored pairing (a pairing in progress is cancelled; the screen starts a fresh invite).
     */
    private fun restartForOnlyLocal(r: Run) {
        if (r.pending != null) {
            cancelPairing()
            return
        }
        r.store.load()?.let { resumeRun(r.role, r.slot, it, monitoring = r.monitoring) }
    }

    @Synchronized
    override fun setTurnServer(url: String, username: String, password: String): TurnSaveResult {
        return when (val r = TurnServerInput.resolve(turn, url, username, password)) {
            TurnServerInput.Resolution.Cleared -> {
                clearTurnServer()
                TurnSaveResult.CLEARED
            }
            is TurnServerInput.Resolution.Invalid ->
                if (r.reason == TurnServerInput.Result.InvalidAddress) {
                    TurnSaveResult.INVALID_ADDRESS
                } else {
                    TurnSaveResult.INVALID_CREDENTIALS
                }
            is TurnServerInput.Resolution.Saved -> {
                turn = r.turn
                _settings.value = withTurn(_settings.value, r.turn)
                turnIo.execute { runCatching { turnStore.save(r.turn) } }
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
        // Taken first: stopping clears the session's choice, and this deletes the one in use.
        settingsRepo.current.role?.let { forgetPairing(it, activeId(it)) }
    }

    private fun forgetPairing(role: Role, id: String) {
        stopMonitoring()
        cancelPairing()
        silence.deactivate()
        babyModel.setSilence(null, silence.hasAccess())
        pairings.forget(role, id)
        settingsRepo.update {
            it.copy(peerName = null, snoozeUntilWallMs = null, pairingId = null)
        }
        parentModel.cancelSnooze()
        refreshApp()
    }

    override fun selectConnection(id: String) {
        val role = settingsRepo.current.role ?: return
        // The choice is per session: it does not change while monitoring runs.
        if (settingsRepo.current.monitoringActive || id !in pairings.savedIds(role)) return
        settingsRepo.update { it.copy(pairingId = id) }
        refreshApp()
    }

    override fun renameConnection(id: String, name: String): Boolean {
        val role = settingsRepo.current.role ?: return false
        if (id !in pairings.savedIds(role)) return false
        val ok = pairings.rename(role, id, name)
        refreshApp()
        return ok
    }

    override fun setDefaultConnection(id: String) {
        val role = settingsRepo.current.role ?: return
        if (pairings.setDefault(role, id)) refreshApp()
    }

    override fun clearDefaultConnection() {
        val role = settingsRepo.current.role ?: return
        pairings.clearDefault(role)
        refreshApp()
    }

    override fun forgetConnection(id: String) {
        val role = settingsRepo.current.role ?: return
        if (id !in pairings.savedIds(role)) return
        if (id == activeId(role)) {
            // Stops monitoring first, then deletes it (the "Forget pairing" requirement).
            forgetPairing(role, id)
            return
        }
        pairings.forget(role, id)
        refreshApp()
    }

    /**
     * MainActivity start/resume: resume a monitoring flag that outlived the process and re-check
     * readiness.
     */
    fun onForeground() {
        val started = autoStartOnce()
        if (
            !started &&
                settingsRepo.current.monitoringActive &&
                (run == null || fgs == Fgs.LOST) &&
                pairingStoreOrNull?.exists() == true
        )
            startHost()
        refreshReadiness()
    }

    /**
     * "When the app opens in parent mode": set once per process (this construction) and cleared by
     * the first thing that settles it, so rotation, returning from the background or a later role
     * change never starts monitoring mid-session. Touched on the main thread only (lifecycle
     * callbacks and UI actions).
     */
    private var autoStartPending = true

    /**
     * Parent only (`device-setup`, "Automatic start with the default connection"): a connection the
     * user marked as default starts monitoring when the app opens. The first [onForeground] with a
     * role settles it (the baby clears it); on the role screen it stays pending for the first
     * [chooseRole]. A running or resumable monitoring flag, an open pairing, unfinished onboarding
     * or a missing marker act on nothing. Returns true when it started monitoring.
     */
    private fun autoStartOnce(): Boolean {
        if (!autoStartPending) return false
        val p = settingsRepo.current
        val role = p.role ?: return false
        autoStartPending = false
        if (role != Role.PARENT || !p.onboardingDone) return false
        if (p.monitoringActive || _pairing.value != null) return false
        val id = pairings.markedDefaultId(Role.PARENT) ?: return false
        selectConnection(id)
        startMonitoring()
        return true
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
        val connections =
            p.role?.let { role ->
                val active = activeId(role)
                pairings.list(role).map {
                    ConnectionUi(it.id, it.name, it.isDefault, it.id == active)
                }
            } ?: emptyList()
        _app.value =
            AppState(
                role = p.role,
                defaultRole = p.defaultRole,
                onboardingDone = p.onboardingDone,
                paired = pairingStoreOrNull?.exists() == true,
                peerName = connections.firstOrNull { it.selected }?.name ?: p.peerName,
                connections = connections,
                // The baby unit replaces its pairing; only the parent adds one, up to the cap.
                canAddPairing = p.role != Role.PARENT || pairings.freeId(Role.PARENT) != null,
                monitoring = p.monitoringActive,
                backgroundProtection = !(p.monitoringActive && fgs == Fgs.LOST),
            )
    }

    // ---- pairing -----------------------------------------------------------------------------

    override fun startPairing() {
        if (settingsRepo.current.role == Role.PARENT) {
            // The parent only scans; no run starts until a code is submitted. At the cap nothing
            // is added (the screens hide the button).
            if (pairings.freeId(Role.PARENT) == null) return
            // "Pair again" from the monitor screen: stop first (user stop), so the scan screen
            // shows at once instead of waiting behind a running session.
            if (settingsRepo.current.monitoringActive) stopMonitoring()
            cancelPairing()
            _pairing.value = PairingState(PairingPhase.SCANNING)
            return
        }
        if (settingsRepo.current.role != Role.BABY) return
        // The baby unit replaces its active pairing; "Add parent device" waits for the baby hub.
        val slot = newPairingId(Role.BABY) ?: return
        cancelPairing()
        stopRun(userStop = true)
        // The baby's own choice for a NEW invite: the settings list (Advanced or the build
        // default); with the off switch on the core's rule leaves the project server out.
        val urls = inviteUrls(_settings.value)
        if (urls.isEmpty()) {
            _pairing.value = PairingState(PairingPhase.FAILED, error = PairingError.UNREACHABLE)
            return
        }
        val psk = ByteArray(PairingPolicy.PSK_BYTES).also(random::nextBytes)
        val now = clock.elapsedMs()
        val payload = PairingPayload.create(psk, urls, clock.wallMs())
        val inviteText = payload.toInviteText()
        if (inviteText == null) {
            // The core refuses an invite for these addresses (e.g. a public ws:// address).
            _pairing.value = PairingState(PairingPhase.FAILED, error = PairingError.UNREACHABLE)
            return
        }
        val pending =
            PendingPairing(
                payload.pskB64url,
                null,
                now,
                now + PairingPolicy.QR_VALIDITY_MS,
                pinUntilS = payload.expiresAtEpochSec,
                mailboxes = urls.toStoredMailboxes(),
            )
        _pairing.value =
            PairingState(
                PairingPhase.SHOWING_CODE,
                codePayload = inviteText,
                expiresAtElapsedMs = pending.expiresAtElapsedMs,
            )
        startRun(
            Role.BABY,
            slot,
            pending,
            pairedBefore = false,
            monitoring = false,
            isPending = true,
        )
    }

    override fun refreshPairingCode() = startPairing()

    override fun cancelPairing() {
        val r = run
        if (r?.pending != null) stopRun(userStop = true)
        _pairing.value = null
    }

    override fun submitPairingCode(payload: String) {
        if (settingsRepo.current.role != Role.PARENT) return
        // At the cap nothing is added until one pairing is forgotten (the screens hide the button).
        val slot = newPairingId(Role.PARENT) ?: return
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
                    slot,
                    PendingPairing(
                        p.pskB64url,
                        null,
                        now,
                        null,
                        pinUntilS = p.expiresAtEpochSec,
                        mailboxes = p.signalingUrls.toStoredMailboxes(),
                    ),
                    pairedBefore = false,
                    monitoring = false,
                    isPending = true,
                )
            }
        }
    }

    // ---- monitoring --------------------------------------------------------------------------

    override fun startMonitoring() {
        // The session's connection stays fixed while monitoring runs, also over a restart.
        val id = settingsRepo.current.role?.let(::activeId)
        settingsRepo.update { it.copy(monitoringActive = true, pairingId = id ?: it.pairingId) }
        refreshApp()
        startHost()
    }

    /** What [ensureRun] found. */
    enum class EnsureRun {
        /** A run exists. */
        RUNNING,

        /** No role or no pairing: monitoring cannot run. */
        NO_PAIRING,

        /** The pairing is kept but unreadable right now; the caller retries once. */
        UNREADABLE,
    }

    /**
     * Called by the service once it is in the foreground: starts (or adopts) the run. Anything but
     * [EnsureRun.RUNNING] while monitoring is persisted active must not leave the service up: the
     * service reports it through [onServiceRunning] (platform fact, same path as a lost service).
     */
    fun ensureRun(): EnsureRun {
        val existing = run
        if (existing != null) {
            promote(existing)
            return EnsureRun.RUNNING
        }
        val role = settingsRepo.current.role ?: return EnsureRun.NO_PAIRING
        val slot = activeId(role)
        return when (val r = pairings.store(role, slot).loadResult()) {
            PairingLoad.Absent -> EnsureRun.NO_PAIRING
            PairingLoad.Unreadable -> EnsureRun.UNREADABLE
            is PairingLoad.Loaded -> {
                resumeRun(role, slot, r.pairing)
                EnsureRun.RUNNING
            }
        }
    }

    private fun promote(r: Run) {
        if (r.monitoring) return
        r.monitoring = true
        if (r.role == Role.BABY) applySilence(_settings.value.silenceWhileMonitoring)
    }

    override fun stopMonitoring() {
        // The next start preselects the default connection again.
        settingsRepo.update { it.copy(monitoringActive = false, pairingId = null) }
        stopRun(userStop = true)
        fgs = Fgs.UNKNOWN
        refreshApp()
        publishFgsAlert()
        host.stop()
    }

    private fun resumeRun(
        role: Role,
        slot: String,
        stored: StoredPairing,
        monitoring: Boolean = true,
    ) =
        startRun(
            role,
            slot,
            PendingPairing(
                stored.pskB64url,
                stored.turn,
                clock.elapsedMs(),
                null,
                parentSecret = stored.parentSecretB64url?.let { PairingPayload.decodePsk(it) },
                mailboxes = stored.mailboxes,
                pinUntilS = stored.pinUntilS,
            ),
            stored.pairedBefore,
            monitoring = monitoring,
            isPending = false,
        )

    /** The URLs of a new invite: the core's `only_local` rule applied to the settings list. */
    private fun inviteUrls(s: SettingsState): List<String> {
        val set = PairingPayload.mailboxesFor(s.signalingUrls)
        val kept = if (s.onlyLocal) filterOnlyLocal(set, PROJECT_HOST) else set
        return kept.map { it.url }
    }

    private fun List<String>.toStoredMailboxes(): List<StoredMailbox> = storedMailboxesFor(this)

    private fun startRun(
        role: Role,
        slot: String,
        pending: PendingPairing,
        pairedBefore: Boolean,
        monitoring: Boolean,
        isPending: Boolean,
    ) {
        stopRun(userStop = false)
        val s = _settings.value
        val coreRole = if (role == Role.BABY) CoreRole.BABY else CoreRole.PARENT
        val config = buildConfig(coreRole, pending.psk, pairedBefore, s, pending)
        val r = Run(++runCounter, role, slot, if (isPending) pending else null, monitoring)
        val ice = buildIceConfig(pending.turn, turn, DebugOptions.relayOnly)
        val runner =
            LinkRunner(
                config = config,
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
            sendEscalation(runner, s)
            refreshReadiness()
        } else {
            sendTalkBack(runner, s)
            if (monitoring) applySilence(s.silenceWhileMonitoring)
        }
    }

    /** The core's escalation delay: set only while discreet and not off, else cleared. */
    private fun sendEscalation(runner: LinkRunner, s: SettingsState) {
        val delay = EscalationPolicy.delayMs(s.alertStyle, s.discreetEscalation)
        runner.input(LinkInput.SetNoiseEscalation(delay?.toULong()))
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
        pending: PendingPairing,
    ): LinkConfig {
        // A fresh seed for every run: the core derives every nonce from it.
        val base = defaultLinkConfig(role, psk, ByteArray(CORE_SEED_BYTES).also(random::nextBytes))
        val heartbeat =
            (DebugOptions.heartbeatMs ?: (s.heartbeatSeconds * 1000L)).coerceIn(5_000L, 15_000L)
        val liveness = 2 * heartbeat + 5_000
        var alarmAfter = DebugOptions.alarmAfterMs ?: AppDefaults.ALARM_AFTER_MS
        // The core rejects a link whose loss detection is not at least 20 s ahead of the alarm.
        if (alarmAfter < liveness + 20_000) alarmAfter = liveness + 20_000
        return base.copy(
            // A copy for the core, which takes it and wipes it; our own bytes stay with the run.
            pinnedParent = pending.parentSecret?.let { ParentSecret(it.copyOf()) },
            pinWindowUntilS = pending.pinUntilS?.toULong(),
            deviceName = if (role == CoreRole.PARENT) deviceName() else "",
            // The mailbox set comes from the pairing record only (owner decision 2026-11-07): the
            // core opens each entry at its own address and the baby also sends the set to the
            // parent after each verification. No fallback to settings or defaults.
            mailboxes = pending.mailboxes.mapNotNull { it.toInviteMailbox() },
            // With the switch on the core itself drops the project server's entries, from this
            // set and from every update.
            onlyLocal = s.onlyLocal,
            projectHost = PROJECT_HOST,
            allowDevLoopback = BuildConfig.DEBUG,
            pairedBefore = pairedBefore,
            heartbeatIntervalMs = heartbeat.toULong(),
            alarmAfterMs = alarmAfter.toULong(),
            noiseThreshold = s.threshold.toDouble(),
            noiseHoldMs = (DebugOptions.noiseHoldMs ?: (s.holdSeconds * 1000L)).toULong(),
        )
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
            alerts.noiseAlert(
                NoiseKind.NEW,
                probe = true,
                snoozed = false,
                discreet = isDiscreet(),
            )
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

    override fun stopAlertSound() {
        parentModel.stopAlertSound()
        // The core's escalation timer counts this as the parent's reaction.
        run?.runner?.input(LinkInput.AcknowledgeNoise)
    }

    override fun requestSilenceBaby() {
        val r = run ?: return
        parentModel.requestSilence(clock.elapsedMs())
        r.runner.input(LinkInput.RequestQuiet)
    }

    override fun tryReconnectNow() {
        run?.runner?.input(LinkInput.NetworkAvailable)
    }

    override fun dismissMissedEvents() = parentModel.dismissMissed()

    override fun setTalk(pressed: Boolean) {
        val r = run?.takeIf { it.role == Role.PARENT } ?: return
        if (!pressed) {
            r.runner.input(LinkInput.SetTalk(false))
            return
        }
        // No permission, no request: the sheet asks first (never during a hold).
        if (!talkGate.micPermission()) {
            parentModel.setTalkPermission(TalkPermission.NEEDED)
            return
        }
        // Over the keyguard nothing may talk to the child.
        if (!talkGate.deviceUnlocked()) return
        parentModel.talkPressed()
        r.runner.input(LinkInput.SetTalk(true))
    }

    override fun interruptTalk() {
        val r = run?.takeIf { it.role == Role.PARENT } ?: return
        if (parentModel.talkActive()) r.runner.input(LinkInput.SetTalk(false))
    }

    override fun talkPermissionResult(granted: Boolean) =
        parentModel.setTalkPermission(
            if (granted) TalkPermission.JUST_GRANTED else TalkPermission.REFUSED
        )

    override fun dismissTalkPermission() = parentModel.setTalkPermission(TalkPermission.NONE)

    override fun dismissTalkCoach() {
        updateSettings { it.copy(talkCoachSeen = true) }
    }

    // ---- baby intents ------------------------------------------------------------------------

    override fun playTalkChimeSample() = chimePreview.play(_settings.value.talkVolume)

    override fun talkSettingsOpened() {
        run?.takeIf { it.role == Role.BABY }?.runner?.input(LinkInput.TalkSettingsOpened)
    }

    private fun sendTalkBack(runner: LinkRunner, s: SettingsState) {
        runner.input(
            LinkInput.SetTalkBack(s.talkBackEnabled, TalkMapping.level(s.talkVolume), s.talkChime)
        )
    }

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
            when (event) {
                is LinkEvent.ParentPinned ->
                    try {
                        persistParentSecret(r, event.secret.bytes, event.windowUntilS?.toLong())
                    } finally {
                        // Our copies are the base64 text in the record and the run; wipe the
                        // event's bytes.
                        event.secret.bytes.fill(0)
                    }
                is LinkEvent.MailboxesUpdated -> persistMailboxes(r, event.mailboxes)
                is LinkEvent.ParentConnected -> nameFromPeer(r, event.name)
                else -> Unit
            }
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

        override fun onTalkMicLevel(rms: Double, nowMs: Long) {
            if (current()) parentModel.onTalkMicLevel(rms, nowMs)
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
                // The baby counts the pairing as complete only when it pinned the parent
                // (`ParentPinned`), not when the link connects: an unpinned baby has no parent.
                LinkState.CONNECTED -> if (r.role == Role.PARENT) completePairing(pending)
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

        /** The record is saved and the pairing link becomes the monitoring run. */
        private fun completePairing(pending: PendingPairing) {
            savePending(pending)
            pending.pinnedSecret?.fill(0)
            pending.pinnedSecret = null
            r.pending = null
            PocLog.event("pairing_saved")
            // The pairing link becomes the monitoring run; the other phone never sees a gap.
            settingsRepo.update { it.copy(monitoringActive = true, pairingId = r.slot) }
            refreshApp()
            startHost()
            // Last, so a UI that reacts to PAIRED already sees a consistent monitoring state.
            _pairing.value = PairingState(PairingPhase.PAIRED)
        }

        /**
         * `ParentPinned`: keep the secret (and the first-pairing window) next to the PSK. This is
         * binding **persist before send**: the parent's core emits it before every action that
         * carries the raw secret and the runner executes the actions of one output in order on this
         * thread, so the record is durable (fsync, then rename) before the baby can have pinned a
         * secret this phone would lose in a restart. A failure throws; the runner then drops the
         * rest of the batch (the secret is not sent) and the core emits the event again with its
         * next resend. The baby pins at this event, which completes its pairing. Never logged.
         */
        private fun persistParentSecret(r: Run, secret: ByteArray, untilS: Long?) {
            val pending = r.pending
            if (pending == null) {
                check(r.store.setParentSecret(PairingPayload.b64(secret), untilS)) {
                    "no pairing record to hold the parent secret"
                }
                return
            }
            pending.pinnedSecret?.fill(0)
            pending.pinnedSecret = secret.copyOf()
            pending.pinUntilS = untilS
            // The attempt is verified at this point, so the pairing is real.
            savePending(pending)
            if (r.role == Role.BABY) completePairing(pending)
        }

        /** Writes the pending pairing as the stored record (the first verification happened). */
        private fun savePending(pending: PendingPairing) {
            val received = pending.receivedMailboxes
            r.store.save(
                StoredPairing(
                    pending.psk,
                    (received ?: pending.mailboxes).signalingUrls(),
                    true,
                    pending.turn,
                    pending.pinnedSecret?.let { PairingPayload.b64(it) },
                    received ?: pending.mailboxes,
                    pending.pinUntilS,
                    // A new pairing has no name of the user's yet.
                    connectionName = pending.peerDeviceName?.let(ConnectionName::fromPeerDevice),
                )
            )
        }

        /** The peer's device name becomes the connection name while the user chose none. */
        private fun nameFromPeer(r: Run, name: String) {
            val pending = r.pending
            if (pending != null) pending.peerDeviceName = name
            else pairings.nameIfUnset(r.role, r.slot, name).also { refreshApp() }
        }

        /** The baby's current mailbox set replaces the stored one. */
        private fun persistMailboxes(r: Run, mailboxes: List<InviteMailbox>) {
            // The core already left the project server out while the off switch is on.
            val set = mailboxes.map { it.toStored() }
            val pending = r.pending
            if (pending != null) pending.receivedMailboxes = set else r.store.setMailboxes(set)
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

        /** The project server's host as the core's `only_local` rule compares it. */
        val PROJECT_HOST: String = PublicLists.projectServer.host.lowercase()

        /** Length of the core CSPRNG seed (`LinkConfig.seed`). */
        const val CORE_SEED_BYTES = 32
    }
}

private fun withTurn(s: SettingsState, turn: TurnCredentials?): SettingsState =
    s.copy(
        turnUrl = turn?.url,
        turnUsername = turn?.username?.takeIf { it.isNotEmpty() },
        turnPasswordSaved = turn?.credential?.isNotEmpty() == true,
    )

/**
 * The engine-wide part of the ICE configuration: the user-entered TURN. The debug launch entry
 * (`pending`) wins over the one saved in the settings. The STUN and relay TURN servers come per
 * attempt from the core.
 */
internal fun buildIceConfig(
    pending: TurnCredentials?,
    stored: TurnCredentials?,
    relayOnly: Boolean,
): IceConfig {
    val t = pending ?: stored
    return IceConfig(
        turnUrl = t?.url,
        turnUser = t?.username?.takeIf { it.isNotEmpty() },
        turnCredential = t?.credential?.takeIf { it.isNotEmpty() },
        relayOnly = relayOnly,
    )
}
