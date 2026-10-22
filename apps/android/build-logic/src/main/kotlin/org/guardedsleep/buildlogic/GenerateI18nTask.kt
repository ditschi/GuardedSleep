package org.guardedsleep.buildlogic

import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

/**
 * Android string resources are generated from the platform-neutral catalogue (the files under
 * locales/).
 */
abstract class GenerateI18nTask : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val localesDir: DirectoryProperty

    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val generatorDir: DirectoryProperty

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
                "i18n-gen",
                "--",
                "android",
                "--out",
                outputDir.get().asFile.absolutePath,
            )
        }
    }
}
