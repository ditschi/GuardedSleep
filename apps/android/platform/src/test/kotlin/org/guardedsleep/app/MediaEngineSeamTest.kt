package org.guardedsleep.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** ADR-020 section 4: the libwebrtc vendor packages stay inside the `media/libwebrtc` package. */
class MediaEngineSeamTest {
    private val vendorImport =
        Regex("""^\s*import\s+(org\.webrtc|io\.getstream)\b""", RegexOption.MULTILINE)
    private val enginePackage = "org/guardedsleep/app/media/libwebrtc/"
    private val sourceExtensions = setOf("kt", "java")

    @Test
    fun vendorPackagesAreOnlyImportedInsideTheEnginePackage() {
        // Unit tests run with the module directory (apps/android/platform) as working directory.
        // Every source set of every module counts (main, debug, test, ...), not only src/main.
        val sources =
            File("..")
                .listFiles()
                .orEmpty()
                .flatMap { File(it, "src").listFiles().orEmpty().toList() }
                .filter { it.isDirectory }
                .flatMap { dir ->
                    dir.walkTopDown()
                        .filter { it.isFile && it.extension in sourceExtensions }
                        .toList()
                }
        assertTrue("no sources found; wrong working directory?", sources.isNotEmpty())
        assertTrue(
            "the debug source set must be scanned",
            sources.any { it.invariantSeparatorsPath.contains("/src/debug/") },
        )
        val offenders =
            sources
                .filter { vendorImport.containsMatchIn(it.readText()) }
                .map { it.invariantSeparatorsPath }
                .filterNot { enginePackage in it }
                .sorted()
        assertEquals(emptyList<String>(), offenders)
    }
}
