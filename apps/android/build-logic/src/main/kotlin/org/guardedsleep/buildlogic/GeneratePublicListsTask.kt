package org.guardedsleep.buildlogic

import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.tasks.CacheableTask
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

/**
 * The curated public relay and STUN lists (the JSON files under `data/`) become a Kotlin object
 * (`PublicLists.kt`) through the Rust `public-lists` generator, which also enforces the build
 * minimums, so a list that is too small fails the build here too.
 */
@CacheableTask
abstract class GeneratePublicListsTask : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val dataDir: DirectoryProperty

    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val generatorDir: DirectoryProperty

    /** A generator dependency bump (Cargo.toml, Cargo.lock) must invalidate the output too. */
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val generatorManifests: ConfigurableFileCollection

    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    @get:Internal abstract val repoRoot: DirectoryProperty

    @get:Inject abstract val exec: ExecOperations

    @TaskAction
    fun generate() {
        val root = repoRoot.get().asFile
        exec.exec {
            workingDir = root
            environment("CARGO_HOME", System.getenv("CARGO_HOME") ?: "${root}/target/.cargo-home")
            environment("CARGO_TARGET_DIR", System.getenv("CARGO_TARGET_DIR") ?: "${root}/target")
            commandLine(
                "bash",
                "scripts/cargo.sh",
                "run",
                "-q",
                "-p",
                "public-lists",
                "--",
                "android",
                "--out",
                outputDir.get().asFile.absolutePath,
            )
        }
    }
}
