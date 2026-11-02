package org.guardedsleep.app

import android.app.Activity
import android.app.LocaleManager
import android.content.Context
import android.content.res.Configuration
import android.content.res.Resources
import android.os.Build
import android.os.LocaleList
import java.util.Locale

/**
 * Applies the in-app language (`SettingsState.language`, a BCP 47 tag from the shipped set, or null
 * for "match system") with the platform per-app locale API.
 *
 * - Android 13+ (API 33): `LocaleManager.setApplicationLocales`. The system stores the choice,
 *   recreates the activities and also uses it for notifications and the system settings entry.
 * - Android 8 to 12: no system API exists without AppCompat, so the activity wraps its base context
 *   with the stored tag ([wrap]) and is recreated when the choice changes. Notifications, channel
 *   names and the foreground service text use `localizedContext` in `platform` instead.
 *
 * Doc: https://developer.android.com/guide/topics/resources/app-languages
 */
object AppLocale {
    fun localesOf(tag: String?): LocaleList =
        if (tag == null) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(tag)

    /** Applies [tag] after the user changed it; a no-op when it is already in effect. */
    fun apply(activity: Activity, tag: String?) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val manager = activity.getSystemService(LocaleManager::class.java) ?: return
            val wanted = localesOf(tag)
            if (manager.applicationLocales != wanted) manager.applicationLocales = wanted
        } else if (effective(tag) != activity.resources.configuration.locales[0]) {
            activity.recreate()
        }
    }

    /** Base context for API 32 and older, so the activity starts in the stored language. */
    fun wrap(base: Context, tag: String?): Context {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU || tag == null) return base
        val config = Configuration(base.resources.configuration)
        val locale = Locale.forLanguageTag(tag)
        Locale.setDefault(locale)
        config.setLocales(LocaleList(locale))
        return base.createConfigurationContext(config)
    }

    private fun effective(tag: String?): Locale =
        if (tag == null) Resources.getSystem().configuration.locales[0]
        else Locale.forLanguageTag(tag)
}
