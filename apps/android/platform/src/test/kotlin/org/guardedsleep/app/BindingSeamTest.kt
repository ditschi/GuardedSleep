package org.guardedsleep.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * AGENTS.md section 5.1: only `platform` uses the UniFFI bindings. The module graph enforces it at
 * compile time; this test keeps the graph itself from being loosened (an `api(...)` or a second
 * dependency on `core-bindings`) and catches imports in source sets that no Gradle task compiles.
 */
class BindingSeamTest {
    private val bindingImport = Regex("""^\s*import\s+uniffi\.""", RegexOption.MULTILINE)
    private val bindingDependency = Regex("""project\(\s*":core-bindings"\s*\)""")
    private val sourceExtensions = setOf("kt", "java")

    // Unit tests run with the module directory (apps/android/platform) as working directory.
    private val modules = File("..").listFiles().orEmpty().filter { File(it, "src").isDirectory }

    @Test
    fun uniffiIsOnlyImportedInsidePlatform() {
        val sources = modules.flatMap { module ->
            File(module, "src")
                .walkTopDown()
                .filter { it.isFile && it.extension in sourceExtensions }
                .toList()
        }
        assertTrue("no sources found; wrong working directory?", sources.isNotEmpty())
        val offenders =
            sources
                .filter { bindingImport.containsMatchIn(it.readText()) }
                .map { it.invariantSeparatorsPath }
                // The generated bindings are core-bindings' own code.
                .filterNot { "/core-bindings/" in it }
                .filterNot { "/platform/" in it }
                .sorted()
        assertEquals(emptyList<String>(), offenders)
    }

    @Test
    fun onlyPlatformDependsOnCoreBindingsAndNeverThroughApi() {
        val builds = modules.map { File(it, "build.gradle.kts") }.filter { it.isFile }
        assertTrue("no build files found; wrong working directory?", builds.size >= 4)
        val dependents =
            builds
                .filter { bindingDependency.containsMatchIn(it.readText()) }
                .map { it.parentFile.name }
        assertEquals(listOf("platform"), dependents)
        assertTrue(
            "platform must not export core-bindings",
            !File("build.gradle.kts").readText().contains("api(project"),
        )
    }
}
