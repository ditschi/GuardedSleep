package org.guardedsleep.app.link

import org.guardedsleep.app.ui.model.ReadinessIssue

/** Starts and stops the foreground service that keeps monitoring alive. */
interface MonitorHost {
    /** Requests the foreground service; throws when Android refuses (the caller must make that visible). */
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
