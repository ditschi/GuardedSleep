package org.guardedsleep.app.silence

import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.SilenceStatus
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.Dnd
import uniffi.guardedsleep_core.Ringer

/** Raw baby `device_status` → the single value the parent sees (ADR-018 §3, alpha-ux-flows §3.12). */
object SilenceMapping {
    /** A browser baby cannot read ringer, DND or access at all, so none of them is ever reported. */
    fun isWebClient(s: DeviceStatus?): Boolean =
        s != null && s.ringer == null && s.dnd == null && s.dndAccess == null

    fun silence(s: DeviceStatus?): SilenceStatus {
        if (s == null || isWebClient(s)) return SilenceStatus.UNKNOWN
        // Our own rule: alarms only, visual effects suppressed.
        if (s.quietActive == true) return SilenceStatus.SILENT
        val dndBlocks = s.dnd == Dnd.ALARMS_ONLY || s.dnd == Dnd.TOTAL
        val outcome =
            when {
                // The user's own Do Not Disturb silences sounds but does not promise a dark screen.
                dndBlocks -> SilenceStatus.SCREEN_MAY_LIGHT
                s.ringer == Ringer.VIBRATE -> SilenceStatus.MAY_BUZZ
                s.ringer == Ringer.NORMAL -> SilenceStatus.MAY_RING
                s.ringer == Ringer.SILENT -> SilenceStatus.SCREEN_MAY_LIGHT
                else -> SilenceStatus.UNKNOWN
            }
        val canMakeNoise = outcome == SilenceStatus.MAY_BUZZ || outcome == SilenceStatus.MAY_RING
        return if (canMakeNoise && s.dndAccess == false) SilenceStatus.NO_ACCESS else outcome
    }

    fun peerStatus(s: DeviceStatus?): PeerStatusUi =
        PeerStatusUi(
            batteryPct = s?.batteryPct?.toInt(),
            charging = s?.charging,
            silence = silence(s),
            alarmsAllowed = s?.alarmsAllowed,
            nextAlarmMinutes = s?.nextAlarmMin?.toInt(),
            micSilent = s?.micSilent,
            isWebClient = isWebClient(s),
            unitWarnings = s?.unitWarnings?.map { it.name.lowercase() }.orEmpty(),
        )
}
