package org.guardedsleep.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Test

/** ADR-020 section 4: the libwebrtc vendor packages stay behind the MediaEngine seam. */
class MediaEngineSeamTest {
    private val vendorPackages = Regex("""\b(org\.webrtc|io\.getstream)\b""")

    @Test
    fun vendorPackagesAreOnlyUsedByTheEngineImplementation() {
        val offenders =
            File("src/main/kotlin")
                .walkTopDown()
                .filter { it.isFile && it.extension == "kt" }
                .filter { vendorPackages.containsMatchIn(it.readText()) }
                .map { it.name }
                .sorted()
                .toList()
        assertEquals(listOf("AttemptPeer.kt", "LibWebRtcMediaEngine.kt"), offenders)
    }
}
