package org.guardedsleep.app.locale

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import java.util.Locale

/**
 * [base] with its resources in the stored app language (`SettingsState.language`, a BCP 47 tag, or
 * null for "match system"), for everything the system shows outside the activity: notification
 * titles and texts, notification channel names and the foreground-service notification.
 *
 * - Android 13+ (API 33): the per-app language (`LocaleManager`) already applies to the whole app,
 *   including notifications, so [base] is returned as it is.
 * - Android 8 to 12: the per-app language only reaches the activity (see `AppLocale.wrap`); the
 *   rest of the process follows the system language, so a configuration context is derived here.
 *
 * Doc: https://developer.android.com/guide/topics/resources/app-languages
 */
// The bundle split is disabled in the `app` module (`bundle { language { enableSplit = false } }`),
// which lint cannot see from this library.
@SuppressLint("AppBundleLocaleChanges")
fun localizedContext(base: Context, storedTag: String?): Context {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU || storedTag == null) return base
    val config = Configuration(base.resources.configuration)
    config.setLocales(LocaleList(Locale.forLanguageTag(storedTag)))
    return base.createConfigurationContext(config)
}
