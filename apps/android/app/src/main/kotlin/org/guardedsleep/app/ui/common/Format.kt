package org.guardedsleep.app.ui.common

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import java.text.DateFormat
import java.util.Date
import java.util.Locale
import org.guardedsleep.app.contract.R

/** `{time}`: short clock time in the user's locale (formatting stays native, ADR-015). */
@Composable
fun formatTime(wallMs: Long): String {
    val env = LocalUiEnv.current
    val locale: Locale = LocalConfiguration.current.locales[0]
    val fmt =
        remember(locale, env.zone) {
            DateFormat.getTimeInstance(DateFormat.SHORT, locale).apply { timeZone = env.zone }
        }
    return fmt.format(Date(wallMs))
}

/** `{duration}`: "45 s", "2 min", "1 h 5 min". */
@Composable
fun formatDuration(ms: Long): String {
    val total = (ms.coerceAtLeast(0) / 1000).toInt()
    return when {
        total < 60 -> stringResource(R.string.native_ui_common_duration_sec, total)
        total < 3600 -> stringResource(R.string.native_ui_common_duration_min, total / 60)
        total % 3600 / 60 == 0 ->
            stringResource(R.string.native_ui_common_duration_hour, total / 3600)
        else ->
            stringResource(
                R.string.native_ui_common_duration_hour_min,
                total / 3600,
                total % 3600 / 60,
            )
    }
}

/**
 * `{duration}` in running text: "48 s", "1 min 48 s", "2 min" (durations in words, never `m:ss`;
 * `redesign-calm-ui` design section 8).
 */
@Composable
fun formatDurationWords(ms: Long): String {
    val total = (ms.coerceAtLeast(0) / 1000).toInt()
    if (total < 60) return stringResource(R.string.native_ui_common_duration_sec, total)
    val min = stringResource(R.string.native_ui_common_duration_min, total / 60)
    return if (total % 60 == 0) min
    else min + " " + stringResource(R.string.native_ui_common_duration_sec, total % 60)
}

/** "1 minute 12 seconds" for the screen reader; the visible counter stays digits. */
@Composable
fun spokenDuration(ms: Long): String {
    val total = (ms.coerceAtLeast(0) / 1000).toInt()
    val minutes = total / 60
    val seconds = total % 60
    val sec = pluralStringResource(R.plurals.native_ui_conn_alert_spoken_sec, seconds, seconds)
    if (minutes == 0) return sec
    val min = pluralStringResource(R.plurals.native_ui_conn_alert_spoken_min, minutes, minutes)
    return if (seconds == 0) min else "$min $sec"
}

/** Countdown / since-contact style "m:ss"; digits are locale independent on purpose. */
fun formatClock(ms: Long): String {
    val total = (ms.coerceAtLeast(0) / 1000)
    return "%d:%02d".format(Locale.ROOT, total / 60, total % 60)
}

fun ceilMinutes(ms: Long): Int = ((ms.coerceAtLeast(0) + 59_999) / 60_000).toInt()
