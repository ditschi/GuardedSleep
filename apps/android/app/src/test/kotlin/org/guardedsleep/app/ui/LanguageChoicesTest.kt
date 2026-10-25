package org.guardedsleep.app.ui

import androidx.test.core.app.ApplicationProvider
import java.io.File
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.languageChoices
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The language list comes from the generated resources, not from a hand-kept list. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class LanguageChoicesTest {
    private val choices =
        ApplicationProvider.getApplicationContext<android.content.Context>().resources.let {
            languageChoices(
                it.getStringArray(R.array.language_tags),
                it.getStringArray(R.array.language_endonyms),
            )
        }

    /** Tags of the catalogues in the `locales` directory, found upwards from the working dir. */
    private fun shippedTags(): List<String> {
        val dir =
            generateSequence(File("").absoluteFile) { it.parentFile }
                .map { File(it, "locales") }
                .first { File(it, "en.json").exists() }
        return dir.listFiles { f -> f.name.endsWith(".json") }!!
            .map { it.name.removeSuffix(".json") }
            .filter { it != "critical" && it != "endonyms" }
            .sorted()
    }

    @Test
    fun listsExactlyTheShippedLocales() {
        assertEquals(shippedTags(), choices.map { it.tag }.sorted())
        assertEquals(20, choices.size)
    }

    @Test
    fun englishComesFirstThenByTag() {
        val tags = choices.map { it.tag }
        assertEquals("en", tags.first())
        assertEquals(tags.drop(1).sorted(), tags.drop(1))
    }

    @Test
    fun showsEndonymsNotTranslations() {
        val byTag = choices.associate { it.tag to it.endonym }
        assertEquals("Deutsch", byTag["de"])
        assertEquals("Português", byTag["pt"])
        assertEquals("中文（简体）", byTag["zh-Hans"])
        assertEquals("日本語", byTag["ja"])
        assertEquals("한국어", byTag["ko"])
        assertEquals("हिन्दी", byTag["hi"])
        assertEquals("العربية", byTag["ar"])
        assertEquals("Türkçe", byTag["tr"])
        assertEquals("Українська", byTag["uk"])
        assertEquals("Bahasa Indonesia", byTag["id"])
        assertTrue(choices.all { it.endonym.isNotBlank() })
        assertNotNull(byTag["en"])
    }
}
