package org.guardedsleep.app.alerts

import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.LinkPhase

enum class NoiseKind {
    /** Full alert: notification, full-screen, vibration, child audio. */
    NEW,

    /** Same event continues within 2 minutes: audio resumes, one short haptic. */
    CONTINUE,
}

/** One-shot parent notices (alpha-ux-flows §4.13): none repeats, none loops. */
sealed interface Notice {
    data object PeerStopped : Notice

    data class LinkRestored(val downMs: Long) : Notice

    data class LowBattery(val pct: Int, val critical: Boolean) : Notice

    data object ChargerUnplugged : Notice

    data object SecurityWarning : Notice

    /** [count] is null unless the baby's summary was complete. */
    data class MissedEvents(val count: Int?, val downMs: Long) : Notice
}

/** What the alert layer needs to know about the link to drive levels 1-3. */
data class LinkSnapshot(
    val phase: LinkPhase,
    val level: Int,
    /** "I'm checking": sound and vibration are muted while true. */
    val acknowledged: Boolean,
    val causeHint: CauseHint?,
    /** The link had verified before; a terminal failure then still alarms (defence in depth). */
    val wasConnected: Boolean = false,
)

/**
 * Everything the parent model can ask the platform to do with the user's attention. The Android
 * implementation is [AlertController]; JVM tests record the calls.
 */
interface AlertSink {
    fun linkState(snapshot: LinkSnapshot)

    /** `LinkLost`: escalation reached level 2 (once per outage). */
    fun linkLost(downMs: Long)

    fun noiseAlert(kind: NoiseKind, probe: Boolean, snoozed: Boolean)

    fun noiseEnded()

    /** "Stop sound": ends sound and vibration of the current noise event. */
    fun stopNoiseSound()

    fun notice(notice: Notice)

    /**
     * Parent: the foreground service is missing while monitoring is on; plays the level 2 feedback
     * until restored.
     */
    fun backgroundProtection(lost: Boolean)

    /** Monitoring ended: cancel every loop and notification. */
    fun clear()
}
