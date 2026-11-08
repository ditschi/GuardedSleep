package org.guardedsleep.app.wiring

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.settings.SettingsRepository

/**
 * Android components of the `app` module that platform notifications start. They are handed in
 * instead of imported, so `platform` never depends on UI code.
 */
interface EntryPoints {
    val mainActivity: Class<out Activity>
    val alertActivity: Class<out Activity>
    val alertActionReceiver: Class<out BroadcastReceiver>
}

/** Intent extras and actions shared by the alert notifications and the `app` components. */
object AlertIntents {
    const val EXTRA_KIND = "kind"
    const val KIND_NOISE = "noise"
    const val KIND_LINK = "link"

    const val ACTION_STOP_SOUND = "org.guardedsleep.app.action.STOP_SOUND"
    const val ACTION_SNOOZE_10 = "org.guardedsleep.app.action.SNOOZE_10"
    const val ACTION_ACK = "org.guardedsleep.app.action.ACK"
    const val ACTION_LISTEN = "org.guardedsleep.app.action.LISTEN"
}

/** What the platform components (monitor service) need from the process-wide object graph. */
interface PlatformServices {
    val settings: SettingsRepository
    val controller: LinkController
    val entryPoints: EntryPoints
}

/** Implemented by the `Application` class of the `app` module. */
interface PlatformServicesProvider {
    val services: PlatformServices
}

fun Context.platformServices(): PlatformServices =
    (applicationContext as PlatformServicesProvider).services
