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
import org.guardedsleep.app.pairing.storedMailboxesFor
import org.guardedsleep.app.ui.model.Role

/**
 * Starts monitoring as if the phones were paired, for the emulator gate. Extras: role
 * (baby|parent), psk_b64url, signal_urls (comma separated), turn_url/turn_user/turn_pass,
 * relay_only, auto_listen, heartbeat_interval_ms, alarm_after_ms, noise_hold_ms, talk_volume. With
 * only `relay_only` set (no role, psk or signal_urls) it changes that one knob and does nothing
 * else: the UI journey (AC-12) pairs through the real screens and needs relay-only media on
 * emulators. Protected by the DUMP permission (adb shell only). The PSK is read from the extras and
 * never logged.
 */
// Debug-only trampoline that finishes in onCreate and never shows a window, so the splash-screen
// advice does not apply.
@SuppressLint("CustomSplashScreen")
class DebugLaunchActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val e = intent.extras
        if (
            e != null &&
                e.containsKey("relay_only") &&
                !e.containsKey("role") &&
                !e.containsKey("psk_b64url") &&
                !e.containsKey("signal_urls")
        ) {
            DebugOptions.setRelayOnly(
                e.getString("relay_only")?.toBooleanStrictOrNull() ?: e.getBoolean("relay_only")
            )
            finish()
            return
        }
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
        // Baby only: the talk-back level of the gate's talk scenario, a percent or low|medium|high
        // (10, 50, 70 percent).
        e.getString("talk_volume")?.let { v ->
            val pct =
                when (v.lowercase()) {
                    "low" -> 10
                    "medium" -> 50
                    "high" -> 70
                    else -> v.toIntOrNull()
                }
            if (pct != null) controller.updateSettings { it.copy(talkVolumePct = pct) }
        }
        controller.stopMonitoring()
        controller.chooseRole(role)
        controller.completePermissionsIntro()
        // The mailbox set of the record, as an invite would have put it there: the core opens
        // nothing that the record does not hold.
        val mailboxes = storedMailboxesFor(urls)
        graph.pairings[role].save(
            StoredPairing(psk, urls, pairedBefore = false, turn = turn, mailboxes = mailboxes)
        )
        controller.startMonitoring()
        finish()
    }
}
