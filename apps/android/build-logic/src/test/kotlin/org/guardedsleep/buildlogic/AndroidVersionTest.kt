package org.guardedsleep.buildlogic

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class AndroidVersionTest {
    @Test
    fun currentAlphaHasAKnownCode() {
        assertEquals(100_101, AndroidVersion.versionCode("0.1.0-alpha.1"))
        assertEquals(100_900, AndroidVersion.versionCode("0.1.0"))
    }

    @Test
    fun codesIncreaseInReleaseOrder() {
        val ordered =
            listOf(
                "0.1.0-alpha.1",
                "0.1.0-alpha.2",
                "0.1.0-alpha.99",
                "0.1.0-beta.1",
                "0.1.0-beta.12",
                "0.1.0-rc.1",
                "0.1.0-rc.99",
                "0.1.0",
                "0.1.1-alpha.1",
                "0.1.1",
                "0.1.99",
                "0.2.0-alpha.1",
                "0.99.99",
                "1.0.0-alpha.1",
                "1.0.0",
                "1.2.3",
                "209.99.99",
            )
        val codes = ordered.map(AndroidVersion::versionCode)
        assertEquals(codes.sorted(), codes)
        assertEquals(codes.distinct(), codes)
        assertTrue(codes.last() < 2_100_000_000)
    }

    @Test
    fun rejectsNamesOutsideTheScheme() {
        listOf(
                "1.0",
                "1.0.0-snapshot",
                "1.0.0-alpha",
                "v1.0.0",
                "0.100.0",
                "0.0.0-alpha.100",
                "210.0.0",
                "0.01.0",
                "0.1.0-alpha.0",
            )
            .forEach { name ->
                try {
                    AndroidVersion.versionCode(name)
                    fail("accepted $name")
                } catch (_: IllegalArgumentException) {} catch (_: IllegalStateException) {}
            }
    }

    // 2026-11-19 14:30:12 UTC
    private val example = "1795098612"

    @Test
    fun devVersionExampleWithPreReleaseBase() {
        val v = AndroidVersion.devVersion("0.1.0-alpha.1", example, "abc12345ff")
        assertEquals("0.1.0-alpha.1.dev.20261119143012+abc12345", v.versionName)
        assertEquals(1795098612 - 1_767_225_600, v.versionCode)
        assertEquals(null, v.warning)
    }

    @Test
    fun devVersionUsesDashWithoutPreReleaseBase() {
        val v = AndroidVersion.devVersion("0.1.0", example, "abc12345")
        assertEquals("0.1.0-dev.20261119143012+abc12345", v.versionName)
    }

    @Test
    fun devVersionCodesOrderByEpoch() {
        val a = AndroidVersion.devVersion("0.1.0", "1795098612", "aaaaaaaa")
        val b = AndroidVersion.devVersion("0.1.0", "1795098613", "00000000")
        assertTrue(b.versionCode > a.versionCode)
    }

    @Test
    fun devVersionFallsBackOnBadInput() {
        val cases =
            listOf(
                Triple("0.1.0", null, "abc12345"),
                Triple("0.1.0", example, null),
                Triple("0.1.0", "", "abc12345"),
                Triple("0.1.0", "12ab", "abc12345"),
                Triple("0.1.0", "-5", "abc12345"),
                Triple("0.1.0", "1767225599", "abc12345"), // before 2026-01-01
                Triple("0.1.0", "1767225600", "abc12345"), // code 0
                Triple("0.1.0", "9999999999", "abc12345"), // above the Play ceiling
                Triple("0.1.0", "99999999999999", "abc12345"), // too long
                Triple("0.1.0", example, "ABC12345"), // upper case
                Triple("0.1.0", example, "xyz12345"), // not hex
                Triple("0.1.0", example, "abc1234"), // shorter than 8
            )
        for ((base, epoch, commit) in cases) {
            val v = AndroidVersion.devVersion(base, epoch, commit)
            assertEquals("$epoch/$commit", 1, v.versionCode)
            assertEquals("$epoch/$commit", "0.1.0-dev.0+unknown", v.versionName)
            assertTrue("$epoch/$commit", v.warning != null)
        }
        assertEquals(
            "0.1.0-alpha.1.dev.0+unknown",
            AndroidVersion.devVersion("0.1.0-alpha.1", null, null).versionName,
        )
    }

    @Test
    fun devVersionCutsLongCommitTo8() {
        val v = AndroidVersion.devVersion("0.1.0", example, "0123456789abcdef")
        assertTrue(v.versionName.endsWith("+01234567"))
    }
}
