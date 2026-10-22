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
                "0.1.0",
                "0.1.1-alpha.1",
                "0.1.1",
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
            )
            .forEach { name ->
                try {
                    AndroidVersion.versionCode(name)
                    fail("accepted $name")
                } catch (_: IllegalArgumentException) {} catch (_: IllegalStateException) {}
            }
    }
}
