package org.guardedsleep.app.ui

import org.guardedsleep.app.ui.model.DurationStrings
import org.guardedsleep.app.ui.model.durationWords
import org.junit.Assert.assertEquals
import org.junit.Test

class DurationWordsTest {
    private val en =
        object : DurationStrings {
            override fun seconds(n: Int) = "$n s"

            override fun minutes(n: Int) = "$n min"
        }

    @Test
    fun readsInWordsAndTruncatesWholeSeconds() {
        assertEquals("0 s", durationWords(0, en))
        assertEquals("48 s", durationWords(48_000, en))
        assertEquals("59 s", durationWords(59_999, en))
        assertEquals("1 min", durationWords(60_000, en))
        assertEquals("1 min 48 s", durationWords(108_000, en))
    }

    @Test
    fun aNegativeDurationReadsAsZero() {
        assertEquals("0 s", durationWords(-5_000, en))
    }
}
