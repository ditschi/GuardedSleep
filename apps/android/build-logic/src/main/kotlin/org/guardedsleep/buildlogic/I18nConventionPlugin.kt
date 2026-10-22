package org.guardedsleep.buildlogic

import com.android.build.api.variant.LibraryAndroidComponentsExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.gradle.kotlin.dsl.register

/** Generates the Android string resources of the module from the catalogue under `locales/`. */
class I18nConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        val root = target.rootProject.layout.projectDirectory.dir("../..")
        val generateI18n =
            target.tasks.register<GenerateI18nTask>("generateI18n") {
                repoRoot.set(root)
                localesDir.set(root.dir("locales"))
                generatorDir.set(root.dir("crates/i18n-gen/src"))
                outputDir.set(target.layout.buildDirectory.dir("generated/i18n/res"))
            }
        target.extensions.configure<LibraryAndroidComponentsExtension> {
            onVariants { variant ->
                variant.sources.res?.addGeneratedSourceDirectory(
                    generateI18n,
                    GenerateI18nTask::outputDir,
                )
            }
        }
    }
}
