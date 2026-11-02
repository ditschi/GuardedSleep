package org.guardedsleep.app.diag

import android.util.Log
import org.guardedsleep.app.platform.BuildConfig

/**
 * Machine-readable events for the emulator gate (`scripts/poc/`), debug builds only. Format:
 * `GS_POC <event> t=<epoch ms> [key=value…]`. Never log IPs, SDPs or keys (guardrail 6).
 */
object PocLog {
    private const val TAG = "GS_POC"

    fun event(name: String, vararg fields: Pair<String, Any?>) {
        if (!BuildConfig.DEBUG) return
        val extra = fields.joinToString("") { (k, v) -> " $k=$v" }
        runCatching { Log.i(TAG, "$name t=${System.currentTimeMillis()}$extra") }
    }
}
