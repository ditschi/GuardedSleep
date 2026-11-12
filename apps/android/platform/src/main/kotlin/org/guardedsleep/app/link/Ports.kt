package org.guardedsleep.app.link

import org.guardedsleep.app.ui.model.ReadinessIssue

/** Starts and stops the foreground service that keeps monitoring alive. */
interface MonitorHost {
    /**
     * Requests the foreground service; throws when Android refuses (the caller must make that
     * visible).
     */
    fun start()

    fun stop()
}

/** Reports when the default network changed or came back (the core retries immediately). */
interface NetworkWatcher {
    fun start(onAvailable: () -> Unit)

    fun stop()
}

fun interface ReadinessProvider {
    fun check(): List<ReadinessIssue>

    /** The parent phone's own battery, shown next to the "plug it in" chip. */
    fun ownBatteryPct(): Int? = null
}

/** What the parent's talk button may do right now (`talk-back`: permission, unlocked device). */
interface TalkGate {
    fun micPermission(): Boolean

    fun deviceUnlocked(): Boolean

    companion object {
        /** No restriction: JVM tests and previews. */
        val Open =
            object : TalkGate {
                override fun micPermission() = true

                override fun deviceUnlocked() = true
            }
    }
}

/** The settings' "Play chime": the chime at the chosen level, outside a run. */
fun interface TalkChimePreview {
    fun play(level: org.guardedsleep.app.ui.model.TalkVolume)
}
