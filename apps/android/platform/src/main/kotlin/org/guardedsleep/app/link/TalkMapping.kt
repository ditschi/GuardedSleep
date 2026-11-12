package org.guardedsleep.app.link

import org.guardedsleep.app.ui.model.TalkDeny
import org.guardedsleep.app.ui.model.TalkEnd
import org.guardedsleep.app.ui.model.TalkVolume
import uniffi.guardedsleep_core.TalkDenyReason
import uniffi.guardedsleep_core.TalkEndReason
import uniffi.guardedsleep_core.TalkLevel

/** Core talk-back vocabulary onto the UI contract (and back for the volume). */
internal object TalkMapping {
    fun end(r: TalkEndReason): TalkEnd =
        when (r) {
            TalkEndReason.RELEASED -> TalkEnd.RELEASED
            TalkEndReason.LIMIT -> TalkEnd.LIMIT
            TalkEndReason.NO_ANSWER -> TalkEnd.NO_ANSWER
            TalkEndReason.LINK_LOST -> TalkEnd.LINK_LOST
            TalkEndReason.MIC_FAILED -> TalkEnd.MIC_FAILED
            TalkEndReason.ALERT -> TalkEnd.ALERT
            TalkEndReason.SWITCHED_OFF -> TalkEnd.SWITCHED_OFF
            TalkEndReason.STOPPED -> TalkEnd.STOPPED
        }

    /** The reason and the retry delay in ms for cooldown and budget. */
    fun deny(r: TalkDenyReason?): Pair<TalkDeny, Long?> =
        when (r) {
            null -> TalkDeny.UNKNOWN to null
            TalkDenyReason.Off -> TalkDeny.OFF to null
            TalkDenyReason.Unsupported -> TalkDeny.UNSUPPORTED to null
            TalkDenyReason.Muted -> TalkDeny.MUTED to null
            TalkDenyReason.Busy -> TalkDeny.BUSY to null
            TalkDenyReason.Rate -> TalkDeny.RATE to null
            is TalkDenyReason.Cooldown -> TalkDeny.COOLDOWN to r.retryInMs.toLong()
            is TalkDenyReason.Budget -> TalkDeny.BUDGET to r.retryInMs.toLong()
        }

    fun level(v: TalkVolume): TalkLevel =
        when (v) {
            TalkVolume.LOW -> TalkLevel.LOW
            TalkVolume.MEDIUM -> TalkLevel.MEDIUM
            TalkVolume.HIGH -> TalkLevel.HIGH
        }
}
