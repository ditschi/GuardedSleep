package org.guardedsleep.app.link

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.LinkUi
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkStatus

/** Maps the core's status onto [BabyMonitorState]. The baby never alerts, so there is no sink. */
class BabyModel(private val clock: Clock) {
    private val _state = MutableStateFlow(BabyMonitorState())
    val state: StateFlow<BabyMonitorState> = _state

    private var last: LinkStatus? = null
    private var micLevel = 0f
    private var zeroSince: Long? = null

    @Synchronized
    fun reset(keepNotice: Boolean = true) {
        last = null
        micLevel = 0f
        zeroSince = null
        _state.value =
            BabyMonitorState(
                threshold = _state.value.threshold,
                holdSeconds = _state.value.holdSeconds,
                dndAccess = _state.value.dndAccess,
                kioskLocked = false,
                silenceRestoredNotice = keepNotice && _state.value.silenceRestoredNotice,
            )
    }

    @Synchronized
    fun onStatus(status: LinkStatus, now: Long) {
        last = status
        publish()
    }

    @Synchronized
    fun onEvent(event: LinkEvent, now: Long) {
        // Only the parent's heartbeat proves the parent is there; nothing else on the baby does.
        when (event) {
            is LinkEvent.NoiseGateChanged ->
                setSettings(event.threshold.toFloat(), (event.holdMs / 1000u).toInt())
            // The parent's listen intent (and its reset when the link is lost) reaches the baby as
            // this event; the screen shows it only when someone wakes the display.
            is LinkEvent.ListenChanged ->
                _state.value = _state.value.copy(parentListening = event.enabled)
            else -> Unit
        }
    }

    @Synchronized
    fun onMicLevel(rms: Double, now: Long) {
        micLevel = rms.coerceIn(0.0, 1.0).toFloat()
        if (rms == 0.0) zeroSince = zeroSince ?: now else zeroSince = null
        publish(now)
    }

    @Synchronized
    fun setSettings(threshold: Float, holdSeconds: Int) {
        _state.value = _state.value.copy(threshold = threshold, holdSeconds = holdSeconds)
    }

    @Synchronized
    fun setSilence(applied: Boolean?, dndAccess: Boolean) {
        _state.value = _state.value.copy(silenceApplied = applied, dndAccess = dndAccess)
    }

    @Synchronized
    fun setKioskLocked(locked: Boolean) {
        _state.value = _state.value.copy(kioskLocked = locked)
    }

    @Synchronized
    fun setSilenceRestoredNotice(show: Boolean) {
        _state.value = _state.value.copy(silenceRestoredNotice = show)
    }

    private fun publish(now: Long = clock.elapsedMs()) {
        val s = last
        val silent = zeroSince?.let { now - it >= MIC_SILENT_MS } == true
        _state.value =
            _state.value.copy(
                link = if (s == null) LinkUi() else LinkMapping.linkUi(s, LinkQuality.UNKNOWN),
                micLevel = micLevel,
                micSilent = silent,
            )
    }

    private companion object {
        /** Same window as the core's `mic_silent` (30 s of digital silence). */
        const val MIC_SILENT_MS = 30_000L
    }
}
