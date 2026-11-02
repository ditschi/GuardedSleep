package org.guardedsleep.app.ui.common

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringArrayResource
import org.guardedsleep.app.contract.R

/** One entry of the language choice: the locale tag and the language's own name. */
data class LanguageChoice(val tag: String, val endonym: String)

/** Pairs the two generated arrays; they are parallel by construction. */
fun languageChoices(tags: Array<String>, endonyms: Array<String>): List<LanguageChoice> {
    check(tags.size == endonyms.size) { "language_tags and language_endonyms differ in length" }
    return tags.zip(endonyms) { tag, name -> LanguageChoice(tag, name) }
}

/**
 * The shipped locales by endonym ("Deutsch"), in the generated order (`en` first, then by tag).
 * Both arrays are generated from `locales/` by i18n-gen (`values/language_choices.xml`), so the
 * list is never kept by hand and cannot drift from what the APK ships. "Match system" is a
 * translated string the screen adds in front.
 */
@Composable
fun rememberLanguageChoices(): List<LanguageChoice> =
    languageChoices(
        stringArrayResource(R.array.language_tags),
        stringArrayResource(R.array.language_endonyms),
    )
