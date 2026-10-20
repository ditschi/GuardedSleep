package org.guardedsleep.app

import android.app.Application

class GuardedSleepApp : Application() {
    lateinit var graph: AppGraph
        private set

    override fun onCreate() {
        super.onCreate()
        graph = AppGraph(this)
    }
}
