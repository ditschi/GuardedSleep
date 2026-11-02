package org.guardedsleep.buildlogic

import com.android.build.api.variant.LibraryAndroidComponentsExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.gradle.kotlin.dsl.register

/**
 * Generates `PublicLists.kt` (curated relays and STUN servers) from the JSON files under `data/`.
 */
class PublicListsConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        val root = target.rootProject.layout.projectDirectory.dir("../..")
        val generate =
            target.tasks.register<GeneratePublicListsTask>("generatePublicLists") {
                repoRoot.set(root)
                dataDir.set(root.dir("data"))
                generatorDir.set(root.dir("crates/public-lists/src"))
                generatorManifests.from(
                    root.file("crates/public-lists/Cargo.toml"),
                    root.file("Cargo.lock"),
                )
                outputDir.set(target.layout.buildDirectory.dir("generated/publicLists/kotlin"))
            }
        target.extensions.configure<LibraryAndroidComponentsExtension> {
            onVariants { variant ->
                variant.sources.kotlin?.addGeneratedSourceDirectory(
                    generate,
                    GeneratePublicListsTask::outputDir,
                )
            }
        }
    }
}
