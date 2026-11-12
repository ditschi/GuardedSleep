package org.guardedsleep.app.link

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.guardedsleep.app.alerts.AlertSink
import org.guardedsleep.app.alerts.FeedbackPlans
import org.guardedsleep.app.alerts.LinkSnapshot
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.Notice
import org.guardedsleep.app.config.AlertPolicy
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.silence.SilenceMapping
import org.guardedsleep.app.ui.model.LinkHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.MissedEventsUi
import org.guardedsleep.app.ui.model.NoisePause
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.SilenceRequest
import org.guardedsleep.app.ui.model.SoundEventUi
import org.guardedsleep.app.ui.model.TalkEnd
import org.guardedsleep.app.ui.model.TalkPermission
import org.guardedsleep.app.ui.model.TalkUi
import org.guardedsleep.app.ui.model.TalkUnavailable
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkStatus
import uniffi.guardedsleep_core.PlaybackResult
import uniffi.guardedsleep_core.TalkAudible
import uniffi.guardedsleep_core.TalkState
import uniffi.guardedsleep_core.TalkStatus
import uniffi.guardedsleep_core.Talker

/** Side effects of the parent model that are not alerts. */
interface ParentEffects {
    /** Playout of the received baby audio (Stop sound, snoozed noise). */
    fun muteRemote(muted: Boolean)

    /** Listen was switched off by the model (60 min limit, outage reset). */
    fun listenStopped()

    /** A probe event arrived: the user's alert chain has just been exercised. */
    fun testAlertArrived(wallMs: Long)

    fun snoozeEnded()
}

/**
 * Maps the core's status and events onto [ParentMonitorState] (ADR-017 §7, alpha-ux-flows §3).
 * Touched by the core thread (events, ticks) and the UI thread (intents), hence synchronized.
 */
class ParentModel(
    private val clock: Clock,
    private val sink: AlertSink,
    private val effects: ParentEffects,
    /** The discreet alert style is on: noise alerts play no audio until the parent listens. */
    private val discreet: () -> Boolean = { false },
    private val threshold: () -> Double,
) {
    private val _state = MutableStateFlow(ParentMonitorState())
    val state: StateFlow<ParentMonitorState> = _state

    private var phase = LinkPhase.IDLE
    private var last: LinkStatus? = null
    private var quality = LinkQuality.UNKNOWN
    private var statsPrev: InboundAudioStats? = null

    private var quietSinceWall: Long? = null
    private var quietBeforeOutageWall: Long? = null
    private var reconnectWall: Long? = null
    private var awaitingMissedUntil: Long? = null
    private var restoredAfterMs: Long? = null
    private var restoredUntil: Long? = null

    private val tracker = NoiseTracker()
    private var noiseSinceWall: Long? = null
    private var noiseLastWall: Long? = null
    private var noiseIsTest = false
    private var remoteLevel = 0.0
    private var remoteAt = Long.MIN_VALUE
    private var heartbeatRms = 0.0
    private var heartbeatAt = Long.MIN_VALUE
    private var lastLoudLevelAt = Long.MIN_VALUE

    /** The running event switched to sound after this many minutes (the core's escalation). */
    private var escalatedAfterMin: Int? = null
    /** "Stop sound" or "Stop vibration" was used since the last noise alert. */
    private var soundStopped = false

    private var listen = ListenMode.OFF
    private var listenSince = 0L
    private var snoozeUntilWall: Long? = null
    private var snoozeUntilElapsed: Long? = null

    private var peerStatus: DeviceStatus? = null
    private var peerStatusAt = 0L

    // Talk-back: the core's state for the button, how the last talk ended, the permission step.
    private var talkUi: TalkUi = TalkUi.Idle
    private var talkEnd: TalkEnd? = null
    private var talkNoticeAt = 0L
    private var talkPermission = TalkPermission.NONE
    private var noisePaused = false
    private var talkUnconfirmed = false
    private var micQuietSince: Long? = null
    private var talkMicSilent = false
    private var afterTalkListenUntil: Long? = null
    private var silenceRequest = SilenceRequest.IDLE
    private var silenceRequestedAt = 0L
    private var silenceDoneAt = 0L

    private var missed: MissedEventsUi? = null
    private var security = false
    private var hints: Set<LinkHint> = emptySet()
    private var readiness: List<ReadinessIssue> = emptyList()
    private var ownBatteryPct: Int? = null
    private var lastSnapshot: LinkSnapshot? = null
    private var everConnected = false
    private var peerAbsentSince: Long? = null

    @Synchronized
    fun reset() {
        phase = LinkPhase.IDLE
        last = null
        quality = LinkQuality.UNKNOWN
        statsPrev = null
        quietSinceWall = null
        quietBeforeOutageWall = null
        reconnectWall = null
        everConnected = false
        peerAbsentSince = null
        awaitingMissedUntil = null
        restoredAfterMs = null
        restoredUntil = null
        tracker.reset()
        noiseSinceWall = null
        noiseLastWall = null
        noiseIsTest = false
        clearNoiseEventFlags()
        remoteLevel = 0.0
        heartbeatRms = 0.0
        heartbeatAt = Long.MIN_VALUE
        listen = ListenMode.OFF
        peerStatus = null
        talkUi = TalkUi.Idle
        talkEnd = null
        noisePaused = false
        talkUnconfirmed = false
        clearMicSilent()
        afterTalkListenUntil = null
        silenceRequest = SilenceRequest.IDLE
        missed = null
        security = false
        hints = emptySet()
        lastSnapshot = null
        publish(clock.elapsedMs())
    }

    @Synchronized
    fun setReadiness(issues: List<ReadinessIssue>, ownBatteryPct: Int? = null) {
        readiness = issues
        this.ownBatteryPct = ownBatteryPct
        publish(clock.elapsedMs())
    }

    /**
     * Restores a snooze that survived a restart; [untilWallMs] is converted to the monotonic clock.
     */
    @Synchronized
    fun restoreSnooze(untilWallMs: Long?) {
        val now = clock.elapsedMs()
        val wall = clock.wallMs()
        if (untilWallMs == null || untilWallMs <= wall) {
            snoozeUntilWall = null
            snoozeUntilElapsed = null
        } else {
            snoozeUntilWall = untilWallMs
            snoozeUntilElapsed = now + (untilWallMs - wall)
        }
        publish(now)
    }

    /** Monotonic end of the active snooze, for `set_snooze`. */
    @Synchronized fun snoozeUntilElapsedMs(): Long? = snoozeUntilElapsed

    @Synchronized
    fun snooze(minutes: Int): Long {
        val now = clock.elapsedMs()
        val wall = clock.wallMs()
        snoozeUntilElapsed = now + minutes * 60_000L
        snoozeUntilWall = wall + minutes * 60_000L
        publish(now)
        return snoozeUntilWall!!
    }

    @Synchronized
    fun cancelSnooze() {
        snoozeUntilElapsed = null
        snoozeUntilWall = null
        publish(clock.elapsedMs())
    }

    // ---- link ------------------------------------------------------------------------------

    @Synchronized
    fun onStatus(status: LinkStatus, now: Long) {
        val prev = phase
        val next = LinkMapping.phase(status.state)
        last = status
        phase = next
        if (next == LinkPhase.CONNECTED) everConnected = true
        if (status.cause == "peer_absent") peerAbsentSince = peerAbsentSince ?: now
        else peerAbsentSince = null
        val wall = clock.wallMs()
        when {
            next == LinkPhase.CONNECTED && prev != LinkPhase.CONNECTED -> {
                statsPrev = null
                reconnectWall = wall
                if (prev == LinkPhase.RECONNECTING) {
                    // After an outage "quiet" must be earned: only a complete summary restores it.
                    quietSinceWall = null
                    awaitingMissedUntil = now + AlertPolicy.MISSED_EVENTS_GRACE_MS
                } else {
                    quietSinceWall = wall
                    quietBeforeOutageWall = null
                    awaitingMissedUntil = null
                }
            }
            next != LinkPhase.CONNECTED && prev == LinkPhase.CONNECTED -> {
                quietBeforeOutageWall = quietSinceWall
                quietSinceWall = null
                awaitingMissedUntil = null
                quality = LinkQuality.UNKNOWN
            }
        }
        if (next != LinkPhase.CONNECTED) quietSinceWall = null
        if (next == LinkPhase.STOPPED) remoteLevel = 0.0
        publishLink(now)
        publish(now)
    }

    @Synchronized
    fun onEvent(event: LinkEvent, now: Long) {
        when (event) {
            is LinkEvent.LinkLost -> sink.linkLost(event.downMs.toLong())
            is LinkEvent.LinkRestored -> {
                restoredAfterMs = event.downMs.toLong()
                restoredUntil = now + AlertPolicy.RESTORED_NOTICE_MS
                sink.notice(Notice.LinkRestored(event.downMs.toLong()))
            }
            is LinkEvent.SecurityWarning -> {
                security = true
                sink.notice(Notice.SecurityWarning)
            }
            LinkEvent.PeerStopped -> sink.notice(Notice.PeerStopped)
            is LinkEvent.AlarmAcknowledged -> publishLink(now)
            is LinkEvent.NoiseAlert -> onNoise(event, now)
            is LinkEvent.NoiseEscalated -> onEscalated(event.afterMs.toLong())
            is LinkEvent.PeerHeartbeat -> {
                heartbeatRms = event.rms ?: 0.0
                heartbeatAt = now
            }
            is LinkEvent.PeerStatus -> onPeerStatus(event.status, now)
            is LinkEvent.LowBattery -> {
                sink.notice(Notice.LowBattery(event.pct.toInt(), event.critical))
            }
            LinkEvent.ChargerUnplugged -> sink.notice(Notice.ChargerUnplugged)
            is LinkEvent.MissedEvents -> onMissed(event, now)
            is LinkEvent.ListenChanged -> {
                if (!event.enabled && listen != ListenMode.OFF) {
                    listen = ListenMode.OFF
                    effects.listenStopped()
                }
                // The core starts a 10 s listen after a talk; its end arrives as `false`.
                if (event.enabled && listen == ListenMode.OFF) {
                    afterTalkListenUntil = now + AlertPolicy.TALK_AFTER_LISTEN_MS
                    effects.muteRemote(false)
                } else if (!event.enabled && afterTalkListenUntil != null) {
                    afterTalkListenUntil = null
                    if (listen == ListenMode.OFF) applyNoisePlayout(NoiseKind.CONTINUE, true)
                }
            }
            is LinkEvent.TalkChanged -> onTalkChanged(event.state, now)
            is LinkEvent.NoisePaused -> noisePaused = event.paused
            is LinkEvent.TalkPlayback ->
                talkUnconfirmed = event.result == PlaybackResult.UNCONFIRMED
            // Baby-side only.
            is LinkEvent.TalkActive -> Unit
            is LinkEvent.Hint ->
                LinkMapping.linkHint(event.key)?.let { hint ->
                    hints = if (event.active) hints + hint else hints - hint
                }
            // Baby-side gate echo; the parent learns the real threshold from peer_status.
            // The pin and the mailbox set are persisted by the controller, not shown.
            is LinkEvent.NoiseGateChanged,
            is LinkEvent.Status,
            is LinkEvent.ParentPinned,
            is LinkEvent.ParentConnected,
            is LinkEvent.MailboxesUpdated,
            is LinkEvent.Failed -> Unit
        }
        publish(now)
    }

    @Synchronized
    fun onRemoteLevel(rms: Double, now: Long) {
        remoteLevel = maxOf(rms, remoteLevel * DECAY)
        remoteAt = now
        tracker.onLevel(now, rms, threshold())
        if (rms >= threshold()) {
            lastLoudLevelAt = now
            if (tracker.active) noiseLastWall = clock.wallMs()
        }
        publish(now)
    }

    /**
     * The level of the parent's own microphone while a talk has it attached. Presentation only:
     * nothing here reaches the core, and the line stays visual (never spoken).
     */
    @Synchronized
    fun onTalkMicLevel(rms: Double, now: Long) {
        if (talkUi !is TalkUi.Talking) return
        if (rms >= AlertPolicy.TALK_MIC_SILENT_BELOW) {
            clearMicSilent()
        } else {
            val since = micQuietSince ?: now.also { micQuietSince = it }
            talkMicSilent = now - since >= AlertPolicy.TALK_MIC_SILENT_MS
        }
        publish(now)
    }

    private fun clearMicSilent() {
        micQuietSince = null
        talkMicSilent = false
    }

    @Synchronized
    fun onInboundStats(stats: InboundAudioStats, now: Long) {
        val prev = statsPrev
        statsPrev = stats
        if (prev != null) {
            LinkMapping.quality(
                    stats.packetsReceived - prev.packetsReceived,
                    stats.packetsLost - prev.packetsLost,
                )
                ?.let { quality = it }
        }
        publishLink(now)
        publish(now)
    }

    @Synchronized
    fun onTick(now: Long) {
        if (now - remoteAt > STALE_AUDIO_MS) remoteLevel *= DECAY
        if (tracker.tick(now)) {
            val wall = clock.wallMs()
            quietSinceWall =
                if (phase == LinkPhase.CONNECTED) wall - (now - tracker.lastLoudElapsedMs) else null
            clearNoiseEventFlags()
            sink.noiseEnded()
        }
        restoredUntil
            ?.takeIf { now >= it }
            ?.let {
                restoredUntil = null
                restoredAfterMs = null
            }
        awaitingMissedUntil
            ?.takeIf { now >= it }
            ?.let {
                // No summary arrived (baby restarted, or older peer): honest fallback is the
                // reconnect time.
                awaitingMissedUntil = null
                quietSinceWall = reconnectWall
            }
        if (talkNoticeAt != 0L && now - talkNoticeAt >= TALK_NOTICE_MS) {
            talkNoticeAt = 0L
            talkEnd = null
            if (talkUi is TalkUi.Denied) talkUi = TalkUi.Idle
        }
        if (listen == ListenMode.LOCKED && now - listenSince >= AlertPolicy.LISTEN_MAX_MS) {
            listen = ListenMode.OFF
            effects.listenStopped()
        }
        if (
            silenceRequest == SilenceRequest.PENDING &&
                now - silenceRequestedAt >= SILENCE_TIMEOUT_MS
        ) {
            silenceRequest = SilenceRequest.FAILED
            silenceDoneAt = now
        }
        if (silenceRequest == SilenceRequest.APPLIED && now - silenceDoneAt >= APPLIED_VISIBLE_MS) {
            silenceRequest = SilenceRequest.IDLE
        }
        snoozeUntilElapsed
            ?.takeIf { now >= it }
            ?.let {
                snoozeUntilElapsed = null
                snoozeUntilWall = null
                effects.snoozeEnded()
                // Noise that is still going at the end of the snooze alerts immediately.
                if (tracker.active && now - lastLoudLevelAt < 3_000) {
                    // A new event: the core restarts its count too, so a past escalation is over.
                    escalatedAfterMin = null
                    applyNoisePlayout(NoiseKind.NEW, snoozed = false)
                    sink.noiseAlert(
                        NoiseKind.NEW,
                        probe = false,
                        snoozed = false,
                        discreet = sinkDiscreet(),
                    )
                }
            }
        publish(now)
    }

    // ---- user intents ------------------------------------------------------------------------

    /** False when the request is ignored (not connected). */
    @Synchronized
    fun setListen(mode: ListenMode, now: Long): Boolean {
        if (phase != LinkPhase.CONNECTED && mode != ListenMode.OFF) return false
        if (mode == ListenMode.LOCKED && listen != ListenMode.LOCKED) listenSince = now
        listen = mode
        if (mode != ListenMode.OFF) afterTalkListenUntil = null
        if (mode != ListenMode.OFF) effects.muteRemote(false)
        // Discreet: ending Listen must not let a running event's audio leak back in.
        else if (discreet() && escalatedAfterMin == null) effects.muteRemote(true)
        publish(now)
        return true
    }

    /** The alert style changed: switching to discreet silences playout at once unless listening. */
    @Synchronized
    fun onAlertStyleChanged() {
        if (discreet()) {
            if (listen == ListenMode.OFF && escalatedAfterMin == null) effects.muteRemote(true)
        } else if (tracker.active && !soundStopped && !snoozeActive()) {
            // Back to sound during a running event: the audio starts at once.
            effects.muteRemote(false)
        }
    }

    private fun snoozeActive(): Boolean = snoozeUntilElapsed?.let { clock.elapsedMs() < it } == true

    @Synchronized
    fun stopAlertSound() {
        soundStopped = true
        effects.muteRemote(true)
        sink.stopNoiseSound()
    }

    /** The talk button went down: the last result is stale now. */
    @Synchronized
    fun talkPressed() {
        talkEnd = null
        talkUnconfirmed = false
        talkNoticeAt = 0L
        if (talkUi is TalkUi.Denied) talkUi = TalkUi.Idle
        publish(clock.elapsedMs())
    }

    @Synchronized
    fun setTalkPermission(p: TalkPermission) {
        talkPermission = p
        publish(clock.elapsedMs())
    }

    /** True while a request or a talk runs (the platform interrupts only then). */
    @Synchronized
    fun talkActive(): Boolean = talkUi is TalkUi.Requesting || talkUi is TalkUi.Talking

    @Synchronized
    fun requestSilence(now: Long) {
        silenceRequest = SilenceRequest.PENDING
        silenceRequestedAt = now
        publish(now)
    }

    @Synchronized
    fun dismissMissed() {
        missed = null
        publish(clock.elapsedMs())
    }

    // ---- internals ---------------------------------------------------------------------------

    private fun onNoise(e: LinkEvent.NoiseAlert, now: Long) {
        val wall = clock.wallMs()
        val kind = tracker.onAlert(now)
        lastLoudLevelAt = now
        if (kind == NoiseKind.NEW || noiseSinceWall == null) {
            noiseSinceWall = wall
            noiseIsTest = e.probe
        }
        if (kind == NoiseKind.NEW) escalatedAfterMin = null
        soundStopped = false
        noiseLastWall = wall
        quietSinceWall = null
        if (e.probe) effects.testAlertArrived(wall)
        // Snoozed or discreet noise stays visual: the baby still streams, but nothing plays until
        // the parent listens.
        applyNoisePlayout(kind, e.snoozed)
        sink.noiseAlert(kind, e.probe, e.snoozed, sinkDiscreet())
    }

    /** Discreet and not yet switched to sound by the core's escalation. */
    private fun sinkDiscreet(): Boolean = discreet() && escalatedAfterMin == null

    private fun playsAudio(kind: NoiseKind, snoozed: Boolean): Boolean =
        FeedbackPlans.forNoise(kind, snoozed, sinkDiscreet()).playAudio

    /** Mutes the received audio unless the plan plays it or the parent is listening. */
    private fun applyNoisePlayout(kind: NoiseKind, snoozed: Boolean) {
        effects.muteRemote(!playsAudio(kind, snoozed) && listen == ListenMode.OFF)
    }

    private fun clearNoiseEventFlags() {
        escalatedAfterMin = null
        soundStopped = false
    }

    /** The core's escalation timer fired: the audio plays as in the sound style. */
    private fun onEscalated(afterMs: Long) {
        // The core's event and the platform's can differ at the edges: only a running event of
        // this unit switches.
        if (!tracker.active) return
        escalatedAfterMin = (afterMs / 60_000L).toInt().coerceAtLeast(1)
        effects.muteRemote(false)
    }

    private fun onTalkChanged(state: TalkState, now: Long) {
        when (state) {
            TalkState.Requesting -> {
                talkUi = TalkUi.Requesting
                talkEnd = null
                talkUnconfirmed = false
                clearMicSilent()
            }
            is TalkState.Talking -> talkUi = TalkUi.Talking(state.untilMs.toLong())
            is TalkState.Ended -> {
                talkUi = TalkUi.Idle
                clearMicSilent()
                talkEnd = TalkMapping.end(state.reason)
                talkNoticeAt = now
            }
            is TalkState.Denied -> {
                val (reason, retry) = TalkMapping.deny(state.reason)
                talkUi = TalkUi.Denied(reason, retry?.let { now + it })
                talkNoticeAt = now
            }
        }
    }

    /** What the baby device's status says about talking, while no talk runs. */
    private fun talkAvailability(now: Long): TalkUi {
        if (talkUi != TalkUi.Idle) return talkUi
        if (phase != LinkPhase.CONNECTED) return TalkUi.Unavailable(TalkUnavailable.NOT_CONNECTED)
        val s = peerStatus ?: return TalkUi.Idle
        val retry = s.talkRetryInMs?.toLong()?.takeIf { it > 0 }?.let { peerStatusAt + it }
        return when {
            s.talk == TalkStatus.OFF -> TalkUi.Unavailable(TalkUnavailable.OFF)
            s.talk == TalkStatus.UNSUPPORTED -> TalkUi.Unavailable(TalkUnavailable.UNSUPPORTED)
            s.talk == TalkStatus.MUTED -> TalkUi.Unavailable(TalkUnavailable.MUTED)
            s.talker == Talker.OTHER -> TalkUi.Unavailable(TalkUnavailable.OTHER_TALKING)
            retry != null && retry > now -> TalkUi.Unavailable(TalkUnavailable.COOLDOWN, retry)
            else -> TalkUi.Idle
        }
    }

    private fun noisePause(): NoisePause? =
        when {
            !noisePaused -> null
            talkUi is TalkUi.Requesting ||
                talkUi is TalkUi.Talking ||
                peerStatus?.talker == Talker.YOU -> NoisePause.YOU
            else -> NoisePause.OTHER
        }

    private fun onPeerStatus(s: DeviceStatus, now: Long) {
        peerStatus = s
        peerStatusAt = now
        if (silenceRequest == SilenceRequest.PENDING && s.quietActive == true) {
            silenceRequest = SilenceRequest.APPLIED
            silenceDoneAt = now
        }
    }

    private fun onMissed(e: LinkEvent.MissedEvents, now: Long) {
        val count = e.count.toInt()
        missed = MissedEventsUi(e.downMs.toLong(), if (e.complete) count else null, e.complete)
        if (awaitingMissedUntil != null) {
            awaitingMissedUntil = null
            val covered = e.complete && count == 0
            quietSinceWall = if (covered) quietBeforeOutageWall ?: reconnectWall else reconnectWall
        }
        sink.notice(Notice.MissedEvents(if (e.complete) count else null, e.downMs.toLong()))
    }

    private fun publishLink(now: Long) {
        val s = last ?: return
        val snapshot =
            LinkSnapshot(
                phase = phase,
                level = LinkMapping.linkUi(s, quality).level,
                acknowledged = s.acknowledgedUntil?.let { now < it.toLong() } == true,
                causeHint = LinkMapping.causeHint(s.cause),
                wasConnected = everConnected,
            )
        if (snapshot != lastSnapshot) {
            lastSnapshot = snapshot
            sink.linkState(snapshot)
        }
    }

    private fun staleHint(now: Long): Boolean =
        peerAbsentSince?.let { now - it > STALE_PAIRING_MS } == true

    private fun meter(now: Long): Float {
        val hb = if (now - heartbeatAt < HEARTBEAT_VISIBLE_MS) heartbeatRms else 0.0
        return maxOf(remoteLevel, hb).coerceIn(0.0, 1.0).toFloat()
    }

    private fun publish(now: Long) {
        val connected = phase == LinkPhase.CONNECTED
        val s = last
        _state.value =
            ParentMonitorState(
                link =
                    if (s == null) LinkUi(hints = hints)
                    else
                        LinkMapping.linkUi(s, quality, hints)
                            .copy(stalePairingHint = staleHint(now)),
                peer = SilenceMapping.peerStatus(peerStatus),
                babyThreshold = peerStatus?.noiseThreshold?.toFloat(),
                level = if (connected) meter(now) else 0f,
                quietSinceWallMs = quietSinceWall,
                noiseAlertActive = tracker.active,
                listen = listen,
                snoozedUntilWallMs = snoozeUntilWall,
                readiness = readiness,
                silenceRequest = silenceRequest,
                missed = missed,
                restoredAfterMs = restoredAfterMs,
                securityWarning = security,
                noiseSinceWallMs = if (tracker.active) noiseSinceWall else null,
                lastSound =
                    noiseSinceWall?.let {
                        SoundEventUi(it, ((noiseLastWall ?: it) - it).coerceAtLeast(0))
                    },
                ownBatteryPct = ownBatteryPct,
                noiseIsTest = noiseIsTest && tracker.active,
                noiseEscalatedAfterMin = escalatedAfterMin.takeIf { tracker.active },
                talk = talkAvailability(now),
                talkEnd = talkEnd,
                talkPermission = talkPermission,
                noisePause = noisePause(),
                talkUnconfirmed = talkUnconfirmed,
                talkAudibleUncertain = peerStatus?.talkAudible == TalkAudible.UNCERTAIN,
                talkMicSilent = talkMicSilent,
                afterTalkListenUntilElapsedMs = afterTalkListenUntil,
            )
    }

    private companion object {
        const val DECAY = 0.85
        const val STALE_PAIRING_MS = 10 * 60_000L
        const val STALE_AUDIO_MS = 400L
        const val HEARTBEAT_VISIBLE_MS = 12_000L
        const val SILENCE_TIMEOUT_MS = 10_000L
        const val APPLIED_VISIBLE_MS = 6_000L
        const val TALK_NOTICE_MS = 8_000L
    }
}
