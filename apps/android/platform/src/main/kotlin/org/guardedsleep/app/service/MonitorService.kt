package org.guardedsleep.app.service

import android.annotation.SuppressLint
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.guardedsleep.app.alerts.StatusNotifier
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.link.MonitorHost
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.wiring.platformServices

/**
 * Keeps monitoring alive in the background. The baby runs as a `microphone` foreground service, the
 * parent as `mediaPlayback`; both hold a partial wake lock because the core's timers must keep
 * ticking with the screen off. Both are sticky: the parent resumes from the stored pairing. A baby
 * restarted by the OS cannot start a microphone service from the background (Android 14+), so it
 * posts a "Tap to resume monitoring" notification instead (ADR-017 §9).
 */
class MonitorService : Service() {
    private var wakeLock: PowerManager.WakeLock? = null
    private var scope: CoroutineScope? = null
    private var wifiLock: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val graph = platformServices()
        val persisted = graph.settings.current
        val role = persisted.role
        if (intent?.action == ACTION_STOP || role == null || !persisted.monitoringActive) {
            stopSelf()
            return START_NOT_STICKY
        }
        val notifier = StatusNotifier(this, graph.entryPoints)
        val type = foregroundType(role)
        try {
            ServiceCompat.startForeground(
                this,
                StatusNotifier.ID_STATUS,
                notifier.build(role, LinkUi()),
                type,
            )
        } catch (e: Exception) {
            PocLog.event("failed", "reason" to "fgs_${e.javaClass.simpleName}")
            if (role == Role.BABY) {
                getSystemService(NotificationManager::class.java)
                    .notify(StatusNotifier.ID_RESUME, notifier.resumePrompt())
            }
            graph.controller.onServiceRunning(false)
            stopSelf()
            return START_NOT_STICKY
        }
        getSystemService(NotificationManager::class.java).cancel(StatusNotifier.ID_RESUME)
        acquireWifiLock()
        if (wakeLock?.isHeld != true) {
            wakeLock =
                getSystemService(PowerManager::class.java)
                    .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "GuardedSleep:monitor")
                    // No timeout: the lock lives exactly as long as this foreground service
                    // (released in onDestroy) and the core's timers must tick through the whole
                    // night.
                    .apply { @SuppressLint("WakelockTimeout") acquire() }
        }
        graph.controller.ensureRun()
        graph.controller.onServiceRunning(true)
        observe(role, notifier)
        return START_STICKY
    }

    // The constants are compile-time ints and the manifest declares the same bits, so the API 29
    // subset check passes; API 26-28 ignore the type.
    @SuppressLint("InlinedApi")
    private fun foregroundType(role: Role): Int =
        if (role == Role.BABY) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        else ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK

    /** Keeps Wi-Fi at full speed with the screen off, so the link does not stall in power save. */
    private fun acquireWifiLock() {
        if (wifiLock?.isHeld == true) return
        val mode =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q)
                WifiManager.WIFI_MODE_FULL_LOW_LATENCY
            else @Suppress("DEPRECATION") WifiManager.WIFI_MODE_FULL_HIGH_PERF
        wifiLock =
            applicationContext
                .getSystemService(WifiManager::class.java)
                ?.createWifiLock(mode, "GuardedSleep:wifi")
                ?.apply {
                    setReferenceCounted(false)
                    runCatching { acquire() }
                }
    }

    private fun observe(role: Role, notifier: StatusNotifier) {
        scope?.cancel()
        val s = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate).also { scope = it }
        val controller = platformServices().controller
        val nm = getSystemService(NotificationManager::class.java)
        if (role == Role.PARENT) {
            s.launch {
                controller.parent
                    .map { it.link }
                    .distinctUntilChanged { a, b ->
                        StatusNotifier.parentText(a) == StatusNotifier.parentText(b)
                    }
                    .collect { nm.notify(StatusNotifier.ID_STATUS, notifier.build(role, it)) }
            }
        }
        s.launch {
            controller.app
                .map { it.monitoring }
                .distinctUntilChanged()
                .collect { monitoring ->
                    if (!monitoring) stopSelf()
                }
        }
    }

    override fun onDestroy() {
        scope?.cancel()
        scope = null
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
        wifiLock?.takeIf { it.isHeld }?.release()
        wifiLock = null
        // Unexpected ends (system stop) show as the red state; a deliberate stop clears the flag
        // first.
        platformServices().controller.onServiceRunning(false)
        PocLog.event("service_stopped")
        super.onDestroy()
    }

    companion object {
        const val ACTION_STOP = "org.guardedsleep.app.STOP"
    }
}

class ServiceLauncher(private val context: Context) : MonitorHost {
    override fun start() {
        // May throw (started from the background); LinkController turns that into the red state.
        ContextCompat.startForegroundService(context, Intent(context, MonitorService::class.java))
    }

    override fun stop() {
        context.stopService(Intent(context, MonitorService::class.java))
    }
}
