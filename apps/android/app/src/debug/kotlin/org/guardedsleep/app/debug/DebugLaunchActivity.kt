package org.guardedsleep.app.debug

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import org.guardedsleep.app.config.DebugOptions
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.graph
import org.guardedsleep.app.pairing.PairingPayload
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.ui.model.Role

/**
 * Starts monitoring as if the phones were paired, for the emulator gate. Extras: role
 * (baby|parent), psk_b64url, signal_urls (comma separated), turn_url/turn_user/turn_pass,
 * relay_only, auto_listen, heartbeat_interval_ms, alarm_after_ms, noise_hold_ms. Protected by the
 * DUMP permission (adb shell only). The PSK is read from the extras and never logged.
 */
// Debug-only trampoline that finishes in onCreate and never shows a window, so the splash-screen
// advice does not apply.
@SuppressLint("CustomSplashScreen")
class DebugLaunchActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val e = intent.extras
        val role =
            when (e?.getString("role")) {
                "baby" -> Role.BABY
                "parent" -> Role.PARENT
                else -> null
            }
        val psk = e?.getString("psk_b64url")?.takeIf { it.isNotBlank() }
        val urls =
            e?.getString("signal_urls")
                ?.split(',')
                ?.map { it.trim() }
                ?.filter { it.isNotEmpty() }
                .orEmpty()
        if (
            role == null ||
                psk == null ||
                urls.isEmpty() ||
                PairingPayload.decodePsk(psk)?.size != 32
        ) {
            PocLog.event("failed", "reason" to "debug_launch_extras")
            finish()
            return
        }
        fun long(key: String) =
            e.getString(key)?.toLongOrNull() ?: e.getLong(key, 0L).takeIf { it > 0 }
        fun bool(key: String) =
            e.getString(key)?.toBooleanStrictOrNull() ?: e.getBoolean(key, false)
        val turn =
            e.getString("turn_url")
                ?.takeIf { it.isNotBlank() }
                ?.let {
                    TurnCredentials(
                        it,
                        e.getString("turn_user").orEmpty(),
                        e.getString("turn_pass").orEmpty(),
                    )
                }
        DebugOptions.set(
            bool("relay_only"),
            bool("auto_listen"),
            long("heartbeat_interval_ms"),
            long("alarm_after_ms"),
            long("noise_hold_ms"),
        )

        val graph = graph()
        val controller = graph.controller
        controller.stopMonitoring()
        controller.chooseRole(role)
        controller.completeOnboarding()
        graph.pairingStore.save(StoredPairing(psk, urls, pairedBefore = false, turn = turn))
        controller.startMonitoring()
        finish()
    }
}
