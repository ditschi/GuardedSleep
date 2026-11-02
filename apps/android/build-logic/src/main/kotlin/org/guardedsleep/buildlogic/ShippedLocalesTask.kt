package org.guardedsleep.buildlogic

import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.CacheableTask
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

/**
 * Writes the locales the catalogue ships (one BCP 47 tag per line) so the app's
 * `androidResources.localeFilters` lists exactly the generated languages and no hand-kept list can
 * drift from `locales/`. A locale that misses a critical key is not shipped, so it is not listed.
 */
@CacheableTask
abstract class ShippedLocalesTask : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val localesDir: DirectoryProperty

    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val generatorDir: DirectoryProperty

    /** A generator dependency bump (Cargo.toml, Cargo.lock) must invalidate the output too. */
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val generatorManifests: ConfigurableFileCollection

    @get:OutputFile abstract val outputFile: RegularFileProperty

    @get:Internal abstract val repoRoot: DirectoryProperty

    @get:Inject abstract val exec: ExecOperations

    @TaskAction
    fun generate() {
        val root = repoRoot.get().asFile
        exec.exec {
            workingDir = root
            commandLine(
                "bash",
                "scripts/cargo.sh",
                "run",
                "-q",
                "-p",
                "i18n-gen",
                "--",
                "locales",
                "--out",
                outputFile.get().asFile.absolutePath,
            )
        }
    }
}
