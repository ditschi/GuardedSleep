package org.guardedsleep.app.link

import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import org.guardedsleep.app.config.AppDefaults
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.media.AttemptListener
import org.guardedsleep.app.media.AttemptPort
import org.guardedsleep.app.media.IceServerEntry
import org.guardedsleep.app.media.InboundAudioStats
import org.guardedsleep.app.media.InjectedAudio
import org.guardedsleep.app.media.MediaEngine
import org.guardedsleep.app.signaling.SignalingFactory
import org.guardedsleep.app.signaling.SignalingListener
import org.guardedsleep.app.signaling.SignalingTransport
import uniffi.guardedsleep_core.CoreLink
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.IceCandidate
import uniffi.guardedsleep_core.IceServer
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
    private val signaling: SignalingFactory,
    private val mediaFactory: () -> MediaEngine,
    /**
     * The "Only same Wi-Fi or my own server" gate: a socket to a refused address is never opened
     * and comes back to the core as closed. It never picks another address.
     */
    private val mailboxAllowed: (String) -> Boolean = { true },
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

    /** One socket per mailbox id, opened and closed only on the core's actions. */
    private class MailboxSocket(val gen: Int, val transport: SignalingTransport)

    private val mailboxes = HashMap<UInt, MailboxSocket>()
    private var mailboxGen = 0
    private val attempts = HashMap<UInt, AttemptPort>()
    private var liveAttempt: UInt? = null
    private val tasks = mutableListOf<ScheduledFuture<*>>()
    private var started = false
    private var closing = false
    private var tornDown = false
    private var lastState: LinkState? = null
    private var lastShape: Triple<LinkState, UByte, String?>? = null
    private var lastSentStatus: DeviceStatus? = null
    private var lastSentStatusAt = 0L
    private var lastWallMs = -1L
    private var lastWallElapsedMs = 0L

    private val isBaby = config.role == Role.BABY

    /** Builds the core and the media engine, opens signaling and starts the timers. */
    fun start() = post {
        // One run per runner: a second start would hand the core the same (wiped) seed.
        if (started) return@post
        started = true
        link =
            try {
                CoreLink(config)
            } catch (e: Exception) {
                PocLog.event("failed", "reason" to "core_config_${e.javaClass.simpleName}")
                teardown()
                return@post
            } finally {
                // The core copied the seed and the parent secret; wipe ours.
                config.seed.fill(0)
                config.pinnedParent?.bytes?.fill(0)
            }
        media = mediaFactory()
        PocLog.event("session_start", "role" to config.role.name.lowercase())
        dispatch(LinkInput.Start(entropy.toULong()))
        reportWallClock()
        every(AppDefaults.TICK_MS) {
            val now = clock.elapsedMs()
            dispatch(LinkInput.Tick)
            reportWallClock()
            listener.onTick(now)
        }
        every(AppDefaults.AUDIO_LEVEL_MS) { sampleAudio() }
        if (isBaby) every(AppDefaults.DEVICE_STATUS_CHECK_MS) { sendDeviceStatus(force = false) }
        else every(STATS_MS) { pollStats() }
    }

    /**
     * The wall clock in Unix seconds, for the `here` message and the clock comparison with the peer
     * (never liveness): sent at start, every 60 s, and when a tick sees the clock jump (more than 2
     * s off the elapsed monotonic time since the last report), which covers a changed device time
     * or time zone.
     */
    private fun reportWallClock() {
        val wallMs = System.currentTimeMillis()
        val elapsed = clock.elapsedMs()
        val sinceMs = elapsed - lastWallElapsedMs
        if (lastWallMs >= 0) {
            val jumpMs = kotlin.math.abs(wallMs - lastWallMs - sinceMs)
            if (sinceMs < WALL_CLOCK_EVERY_MS && jumpMs <= WALL_CLOCK_JUMP_MS) return
        }
        lastWallMs = wallMs
        lastWallElapsedMs = elapsed
        dispatch(LinkInput.WallClock((wallMs / 1000).toULong()))
    }

    /** Announces a deliberate stop to the peer, then tears everything down after a short flush. */
    fun stop() = post {
        if (closing || tornDown) return@post
        closing = true
        dispatch(LinkInput.Stop)
        schedule(FLUSH_MS + 200) { teardown() }
    }

    /** Immediate teardown without telling the peer (process shutdown paths). */
    fun abort() = post { teardown() }

    fun input(input: LinkInput) = post { if (!closing) dispatch(input) }

    fun media(action: (MediaEngine) -> Unit) = post { media?.let(action) }

    /**
     * Runs a task on the core thread (used by the controller for model changes that need ordering).
     */
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
        try {
            for (a in actions) execute(a, now)
        } catch (e: PinNotPersisted) {
            // The secret is not durable, so nothing after the event may run (the message that
            // carries it follows in this very batch). The core repeats the event on its next
            // resend.
            PocLog.event("failed", "reason" to "pin_not_persisted")
        }
    }

    private fun execute(action: LinkAction, now: Long) {
        when (action) {
            is LinkAction.OpenMailbox -> openMailbox(action.id, action.url)
            is LinkAction.CloseMailbox -> closeMailbox(action.id)
            is LinkAction.MailboxSend -> mailboxes[action.id]?.transport?.send(action.frame)
            is LinkAction.CreateAttempt ->
                createAttempt(action.attempt, action.offerer, action.iceServers)
            is LinkAction.CloseAttempt -> closeAttempt(action.attempt)
            is LinkAction.CreateOffer -> attempts[action.attempt]?.createOffer()
            is LinkAction.SetRemoteDescription ->
                attempts[action.attempt]?.setRemoteDescription(action.sdpType, action.sdp)
            is LinkAction.CreateAnswer -> attempts[action.attempt]?.createAnswer()
            is LinkAction.AddIceCandidate ->
                attempts[action.attempt]?.addRemoteIce(action.candidate)
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
                    if (s.state == LinkState.CONNECTED && lastState != LinkState.CONNECTED)
                        PocLog.event("pairing_verified")
                    if (s.state == LinkState.FAILED_NEEDS_USER && lastState != s.state) {
                        PocLog.event("failed", "reason" to (s.key ?: "failed_needs_user"))
                    }
                    lastShape = shape
                    lastState = s.state
                }
                listener.onStatus(s, now)
            }
            is LinkEvent.NoiseAlert ->
                if (!event.snoozed)
                    PocLog.event(
                        "alert",
                        "kind" to "noise",
                        "rms" to event.rms,
                        "probe" to event.probe,
                    )
            is LinkEvent.LinkLost ->
                PocLog.event("alert", "kind" to "link_lost", "down_ms" to event.downMs)
            is LinkEvent.LinkRestored -> PocLog.event("link_restored", "down_ms" to event.downMs)
            is LinkEvent.Failed -> PocLog.event("failed", "reason" to event.reason)
            else -> Unit
        }
        if (event !is LinkEvent.Status) {
            try {
                listener.onEvent(event, now)
            } catch (e: Exception) {
                if (event is LinkEvent.ParentPinned) throw PinNotPersisted() else throw e
            }
        }
    }

    /** The parent secret could not be stored; the exception carries no secret. */
    private class PinNotPersisted : RuntimeException("parent secret not persisted")

    /**
     * Opens exactly the address the core named for mailbox [id]. A failed open comes back as
     * `MailboxClosed`, never as a retry.
     */
    private fun openMailbox(id: UInt, url: String) {
        closeMailbox(id)
        val gen = ++mailboxGen
        if (!mailboxAllowed(url)) {
            // Not a socket at all; the core applies its own backoff.
            post { dispatch(LinkInput.MailboxClosed(id, "blocked")) }
            return
        }
        val transport =
            signaling.create(
                url,
                object : SignalingListener {
                    override fun onOpened() = post {
                        if (isCurrent(id, gen)) dispatch(LinkInput.MailboxOpened(id))
                    }

                    override fun onMessage(text: String) = post {
                        // A frame over the cap is dropped here (a public relay is untrusted); the
                        // core caps too.
                        if (isCurrent(id, gen) && text.length <= MAX_FRAME_CHARS) {
                            dispatch(LinkInput.MailboxFrame(id, text))
                        }
                    }

                    override fun onClosed() = post {
                        if (isCurrent(id, gen)) dispatch(LinkInput.MailboxClosed(id, "closed"))
                    }
                },
            )
        mailboxes[id] = MailboxSocket(gen, transport)
        transport.open()
    }

    /** Late callbacks of a closed or replaced socket carry an old generation and are dropped. */
    private fun isCurrent(id: UInt, gen: Int) = mailboxes[id]?.gen == gen

    private fun closeMailbox(id: UInt) {
        mailboxes.remove(id)?.transport?.close()
    }

    private fun createAttempt(handle: UInt, offerer: Boolean, iceServers: List<IceServer>) {
        val m = media ?: return
        val l =
            object : AttemptListener {
                override fun onLocalDescription(type: String, sdp: String) =
                    forAttempt(handle) {
                        dispatch(LinkInput.AttemptLocalDescription(handle, type, sdp))
                    }

                override fun onLocalIce(candidate: IceCandidate) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptLocalIce(handle, candidate)) }

                override fun onDataOpen() =
                    forAttempt(handle) { dispatch(LinkInput.AttemptDataOpen(handle)) }

                override fun onDataMessage(text: String) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptDataMessage(handle, text)) }

                override fun onDataClosed() =
                    forAttempt(handle) { dispatch(LinkInput.AttemptDataClosed(handle)) }

                override fun onDtlsFingerprint(fingerprint: String) =
                    forAttempt(handle) {
                        dispatch(LinkInput.AttemptDtlsFingerprint(handle, fingerprint))
                    }

                override fun onConnectionState(state: String) =
                    forAttempt(handle) { dispatch(LinkInput.AttemptPcState(handle, state)) }
            }
        attempts[handle] =
            try {
                m.createAttempt(
                    offerer,
                    l,
                    iceServers.map { IceServerEntry(it.urls, it.username, it.credential) },
                )
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

    private fun forAttempt(handle: UInt, block: () -> Unit) = post {
        if (attempts.containsKey(handle) && !tornDown) block()
    }

    private fun teardown() {
        if (tornDown) return
        tornDown = true
        tasks.forEach { it.cancel(false) }
        tasks.clear()
        mailboxes.values.forEach { it.transport.close() }
        mailboxes.clear()
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
        } catch (_: RejectedExecutionException) {}
    }

    private fun every(periodMs: Long, task: () -> Unit) {
        tasks +=
            executor.scheduleWithFixedDelay(
                { post(task) },
                periodMs,
                periodMs,
                TimeUnit.MILLISECONDS,
            )
    }

    private fun DeviceStatus.rounded() =
        copy(batteryPct = batteryPct?.let { (it.toInt() / 5 * 5).toUByte() })

    private companion object {
        const val FLUSH_MS = 300L
        const val STATS_MS = 5_000L

        /** The longest mailbox frame passed on to the core (`signaling-mailboxes`: 64 KiB). */
        const val MAX_FRAME_CHARS = 64 * 1024

        /** The wall clock is reported this often (the core needs minute resolution). */
        const val WALL_CLOCK_EVERY_MS = 60_000L

        /** A wall clock this far off the monotonic time since the last report is a time change. */
        const val WALL_CLOCK_JUMP_MS = 2_000L
    }
}
