package org.guardedsleep.app.ui.model

import android.content.res.Resources
import org.guardedsleep.app.contract.R

/** The two catalogue messages a duration in words is built from. */
interface DurationStrings {
    fun seconds(n: Int): String

    fun minutes(n: Int): String
}

/**
 * `{duration}` in running text: "48 s", "1 min 48 s", "2 min". Durations are in words, never `m:ss`
 * (`redesign-calm-ui` design section 8). Whole seconds are truncated, a negative value reads as 0
 * s. The composable wrapper and the notifications share this one rule.
 */
fun durationWords(ms: Long, str: DurationStrings): String {
    val total = (ms.coerceAtLeast(0) / 1000).toInt()
    if (total < 60) return str.seconds(total)
    val min = str.minutes(total / 60)
    return if (total % 60 == 0) min else min + " " + str.seconds(total % 60)
}

/** The catalogue's `durationSec` and `durationMin` messages of [this] resources. */
fun Resources.durationStrings(): DurationStrings =
    object : DurationStrings {
        override fun seconds(n: Int) =
            getString(R.string.native_ui_common_duration_sec, n.toString())

        override fun minutes(n: Int) =
            getString(R.string.native_ui_common_duration_min, n.toString())
    }
