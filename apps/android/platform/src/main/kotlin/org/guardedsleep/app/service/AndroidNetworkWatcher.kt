package org.guardedsleep.app.service

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import org.guardedsleep.app.link.NetworkWatcher

/** Tells the core when the default network appears or changes, so it retries immediately. */
class AndroidNetworkWatcher(context: Context) : NetworkWatcher {
    private val cm = context.getSystemService(ConnectivityManager::class.java)
    private var callback: ConnectivityManager.NetworkCallback? = null

    @Synchronized
    override fun start(onAvailable: () -> Unit) {
        stop()
        val cb =
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) = onAvailable()
            }
        runCatching { cm.registerDefaultNetworkCallback(cb) }.onSuccess { callback = cb }
    }

    @Synchronized
    override fun stop() {
        callback?.let { runCatching { cm.unregisterNetworkCallback(it) } }
        callback = null
    }
}
