package org.guardedsleep.app.link

import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import org.guardedsleep.app.config.AppDefaults
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.AttemptListener
import org.guardedsleep.app.media.AttemptPort
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.media.InjectedAudio
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.signaling.SignalingListener
import org.guardedsleep.app.signaling.SignalingTransport
import uniffi.guardedsleep_core.CoreLink
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.IceCandidate
import uniffi.guardedsleep_core.LinkAction
import uniffi.guardedsleep_core.LinkConfig
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkInput
import uniffi.guardedsleep_core.LinkState
import uniffi.guardedsleep_core.LinkStatus
import uniffi.guardedsleep_core.Role

/** Called on the `gs-core` thread. */
interface LinkListener {
    fun onStatus(status: LinkStatus, nowMs: Long)

    fun onEvent(event: LinkEvent, nowMs: Long)

    fun onTick(nowMs: Long)

    /** Baby: microphone level for the meter, ~10 Hz. */
    fun onMicLevel(rms: Double, nowMs: Long)

    /** Parent: level of the received audio, ~10 Hz while audio flows. */
    fun onRemoteLevel(rms: Double, nowMs: Long)

    /** Parent: inbound audio counters every few seconds. */
    fun onInboundStats(stats: InboundAudioStats, nowMs: Long)

    /** The run is fully torn down. */
    fun onTornDown()
}

/**
 * Executes one monitoring run of the Rust core ([CoreLink]) on a single thread: every core call and
 * every action runs on [executor], so the core needs no locking. All WebRTC and OkHttp callbacks
 * hop onto it. The runner contains no decisions; it only translates between the core's actions and
 * the platform ports.
 */
class LinkRunner(
    private val config: LinkConfig,
    private val signalingUrls: List<String>,
    private val signaling: SignalingFactory,
    private val mediaFactory: () -> MediaEngine,
    private val clock: Clock,
    private val executor: ScheduledExecutorService,
    private val listener: LinkListener,
    /** Baby: current phone state; null on the parent. */
    private val deviceStatus: (() -> DeviceStatus)? = null,
    /** Baby: switch the phone to silent, return the fresh state. */
    private val applyQuiet: (() -> DeviceStatus)? = null,
    private val entropy: Long,
) {
    private lateinit var link: CoreLink
    private var media: MediaEngine? = null
    private var transport: SignalingTransport? = null
    private var transportGen = 0
    private val attempts = HashMap<UInt, AttemptPort>()
    private var liveAttempt: UInt? = null
    private val tasks = mutableListOf<ScheduledFuture<*>>()
    private var closing = false
    private var tornDown = false
    private var lastState: LinkState? = null
    private var lastShape: Triple<LinkState, UByte, String?>? = null
    private var lastSentStatus: DeviceStatus? = null
    private var lastSentStatusAt = 0L

    private val isBaby = config.role == Role.BABY

    /** Builds the core and the media engine, opens signaling and starts the timers. */
    fun start() =
        post {
            link =
                try {
                    CoreLink(config)
                } catch (e: Exception) {
                    PocLog.event("failed", "reason" to "core_config_${e.javaClass.simpleName}")
                    teardown()
                    return@post
                }
            media = mediaFactory()
            PocLog.event("session_start", "role" to config.role.name.lowercase())
            dispatch(LinkInput.Start(entropy.toULong()))
            every(AppDefaults.TICK_MS) {
                val now = clock.elapsedMs()
                dispatch(LinkInput.Tick)
                listener.onTick(now)
            }
            every(AppDefaults.AUDIO_LEVEL_MS) { sampleAudio() }
            if (isBaby) every(AppDefaults.DEVICE_STATUS_CHECK_MS) { sendDeviceStatus(force = false) }
            else every(STATS_MS) { pollStats() }
        }

    /** Announces a deliberate stop to the peer, then tears everything down after a short flush. */
    fun stop() =
        post {
            if (closing || tornDown) return@post
            closing = true
            dispatch(LinkInput.Stop)
            schedule(FLUSH_MS + 200) { teardown() }
        }

    /** Immediate teardown without telling the peer (process shutdown paths). */
    fun abort() = post { teardown() }

    fun input(input: LinkInput) = post { if (!closing) dispatch(input) }

    fun media(action: (MediaEngine) -> Unit) = post { media?.let(action) }

    /** Runs a task on the core thread (used by the controller for model changes that need ordering). */
    fun run(task: () -> Unit) = post(task)

    private fun sampleAudio() {
        val m = media ?: return
        val now = clock.elapsedMs()
        if (isBaby) {
            val rms = m.drainMicRms() ?: InjectedAudio.level()
            if (rms != null) {
                listener.onMicLevel(rms, now)
                dispatch(LinkInput.AudioLevel(rms))
            }
        } else {
            m.drainRemoteRms()?.let { listener.onRemoteLevel(it, now) }
        }
    }

    private fun sendDeviceStatus(force: Boolean) {
        val read = deviceStatus ?: return
        val now = clock.elapsedMs()
        val status = runCatching(read).getOrNull() ?: return
        val changed = status.rounded() != lastSentStatus?.rounded()
        if (force || changed || now - lastSentStatusAt >= AppDefaults.DEVICE_STATUS_RESEND_MS) {
            lastSentStatus = status
            lastSentStatusAt = now
            dispatch(LinkInput.DeviceStatus(status))
        }
    }

    private fun pollStats() {
        val attempt = liveAttempt?.let { attempts[it] } ?: return
        attempt.inboundAudioStats { stats ->
            if (stats != null) post { listener.onInboundStats(stats, clock.elapsedMs()) }
        }
    }

    private fun dispatch(input: LinkInput) {
        if (tornDown) return
        val now = clock.elapsedMs()
        val actions =
            try {
                link.handle(now.toULong(), input)
            } catch (e: Exception) {
                PocLog.event("failed", "reason" to "core_${e.javaClass.simpleName}")
                return
            }
        for (a in actions) execute(a, now)
    }

    private fun execute(action: LinkAction, now: Long) {
        when (action) {
            LinkAction.OpenSignaling -> openSignaling()
            LinkAction.CloseSignaling -> closeSignaling()
            is LinkAction.SendSignaling -> transport?.send(action.text)
            is LinkAction.CreateAttempt -> createAttempt(action.attempt, action.offerer)
            is LinkAction.CloseAttempt -> closeAttempt(action.attempt)
            is LinkAction.CreateOffer -> attempts[action.attempt]?.createOffer()
            is LinkAction.SetRemoteDescription ->
                attempts[action.attempt]?.setRemoteDescription(action.sdpType, action.sdp)
            is LinkAction.CreateAnswer -> attempts[action.attempt]?.createAnswer()
            is LinkAction.AddIceCandidate -> attempts[action.attempt]?.addRemoteIce(action.candidate)
            is LinkAction.SendData -> attempts[action.attempt]?.sendData(action.text)
            is LinkAction.SetMicEnabled -> media?.setMicEnabled(action.enabled)
            LinkAction.ApplyQuiet -> applyQuietNow()
            is LinkAction.SendProbeTone -> media?.startProbeTone(action.durationMs.toLong())
            is LinkAction.Emit -> emit(action.event, now)
        }
    }

    private fun emit(event: LinkEvent, now: Long) {
        when (event) {
            is LinkEvent.Status -> {
                val s = event.status
                val shape = Triple(s.state, s.level, s.cause)
                if (shape != lastShape) {
                    PocLog.event(
                        "state",
                        "state" to s.state.name.lowercase(),
                        "level" to s.level,
                        "cause" to s.cause,
                    )
                    if (s.state == LinkState.CONNECTED && lastState != LinkState.CONNECTED) PocLog.event("pairing_verified")
                    if (s.state == LinkState.FAILED_NEEDS_USER && lastState != s.state) {
                        PocLog.event("failed", "reason" to (s.key ?: "failed_needs_user"))
                    }
                    lastShape = shape
                    lastState = s.state
                }
                listener.onStatus(s, now)
            }
            is LinkEvent.NoiseAlert ->
                if (!event.snoozed) PocLog.event("alert", "kind" to "noise", "rms" to event.rms, "probe" to event.probe)
            is LinkEvent.LinkLost -> PocLog.event("alert", "kind" to "link_lost", "down_ms" to event.downMs)
            is LinkEvent.LinkRestored -> PocLog.event("link_restored", "down_ms" to event.downMs)
            is LinkEvent.Failed -> PocLog.event("failed", "reason" to event.reason)
            else -> Unit
        }
        if (event !is LinkEvent.Status) listener.onEvent(event, now)
    }

    private fun openSignaling() {
        closeSignaling()
        val gen = ++transportGen
        transport =
            signaling.create(
                signalingUrls,
                object : SignalingListener {
                    override fun onOpened() = post { if (gen == transportGen) dispatch(LinkInput.SignalingOpened) }

                    override fun onMessage(text: String) =
                        post { if (gen == transportGen) dispatch(LinkInput.SignalingMessage(text)) }

                    override fun onClosed() = post { if (gen == transportGen) dispatch(LinkInput.SignalingClosed) }
                },
            )
        transport?.open()
    }

    private fun closeSignaling() {
        transportGen++
        transport?.close()
        transport = null
    }

    private fun createAttempt(handle: UInt, offerer: Boolean) {
        val m = media ?: return
        val l =
            object : AttemptListener {
                override fun onLocalDescription(type: String, sdp: String) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptLocalDescription(handle, type, sdp)) }

                override fun onLocalIce(candidate: IceCandidate) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptLocalIce(handle, candidate)) }

                override fun onDataOpen() = forAttempt(handle) { dispatch(LinkInput.AttemptDataOpen(handle)) }

                override fun onDataMessage(text: String) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptDataMessage(handle, text)) }

                override fun onDataClosed() = forAttempt(handle) { dispatch(LinkInput.AttemptDataClosed(handle)) }

                override fun onDtlsFingerprint(fingerprint: String) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptDtlsFingerprint(handle, fingerprint)) }

                override fun onConnectionState(state: String) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptPcState(handle, state)) }
            }
        attempts[handle] =
            try {
                m.createAttempt(offerer, l)
            } catch (e: Exception) {
                PocLog.event("failed", "reason" to "create_attempt_${e.javaClass.simpleName}")
                // The core sees a failed attempt and schedules the next one.
                post { dispatch(LinkInput.AttemptPcState(handle, "failed")) }
                return
            }
        liveAttempt = handle
    }

    private fun closeAttempt(handle: UInt) {
        val peer = attempts[handle] ?: return
        if (liveAttempt == handle) liveAttempt = null
        if (closing) {
            // A stop_notice was just written to the DataChannel; let it leave before the close.
            schedule(FLUSH_MS) {
                attempts.remove(handle)
                peer.close()
            }
        } else {
            attempts.remove(handle)
            peer.close()
        }
    }

    private fun applyQuietNow() {
        val apply = applyQuiet ?: return
        val state = runCatching(apply).getOrNull() ?: return
        lastSentStatus = state
        lastSentStatusAt = clock.elapsedMs()
        dispatch(LinkInput.QuietApplied(state))
    }

    private fun forAttempt(handle: UInt, block: () -> Unit) =
        post { if (attempts.containsKey(handle) && !tornDown) block() }

    private fun teardown() {
        if (tornDown) return
        tornDown = true
        tasks.forEach { it.cancel(false) }
        tasks.clear()
        transport?.close()
        transport = null
        attempts.values.forEach { runCatching { it.close() } }
        attempts.clear()
        runCatching { media?.release() }
        media = null
        runCatching { link.close() }
        listener.onTornDown()
        executor.shutdown()
    }

    private fun post(task: () -> Unit) {
        try {
            executor.execute {
                try {
                    task()
                } catch (e: Exception) {
                    PocLog.event("failed", "reason" to "runner_${e.javaClass.simpleName}")
                }
            }
        } catch (_: RejectedExecutionException) {
            // Already shut down: late callbacks are dropped.
        }
    }

    private fun schedule(delayMs: Long, task: () -> Unit) {
        try {
            executor.schedule({ post(task) }, delayMs, TimeUnit.MILLISECONDS)
        } catch (_: RejectedExecutionException) {
        }
    }

    private fun every(periodMs: Long, task: () -> Unit) {
        tasks += executor.scheduleWithFixedDelay({ post(task) }, periodMs, periodMs, TimeUnit.MILLISECONDS)
    }

    private fun DeviceStatus.rounded() = copy(batteryPct = batteryPct?.let { (it.toInt() / 5 * 5).toUByte() })

    private companion object {
        const val FLUSH_MS = 300L
        const val STATS_MS = 5_000L
    }
}
