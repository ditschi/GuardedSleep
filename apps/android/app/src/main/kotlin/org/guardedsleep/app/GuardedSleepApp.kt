package org.guardedsleep.app

import android.app.Application
import org.guardedsleep.app.wiring.PlatformServices
import org.guardedsleep.app.wiring.PlatformServicesProvider

class GuardedSleepApp : Application(), PlatformServicesProvider {
    lateinit var graph: AppGraph
        private set

    override val services: PlatformServices
        get() = graph

    override fun onCreate() {
        super.onCreate()
        graph = AppGraph(this)
    }
}
