package org.guardedsleep.buildlogic

import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.artifacts.VersionCatalogsExtension
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.Sync
import org.gradle.api.tasks.testing.Test
import org.gradle.kotlin.dsl.register
import org.gradle.kotlin.dsl.withType
import org.gradle.process.CommandLineArgumentProvider

/**
 * Runs Robolectric offline. Robolectric's own downloader (`MavenArtifactFetcher`) has no retries
 * and bypasses the Gradle cache, and Maven Central redirects the ~150 MB android-all jars to a
 * mirror that fails often. Instead Gradle resolves the jars listed under `robolectric-android-all-*`
 * in the version catalog as ordinary dependencies and copies them into one directory that
 * Robolectric reads through its documented offline mode (`robolectric.offline`,
 * `robolectric.dependency.dir`). Doc: https://robolectric.org/configuring/ (system properties table:
 * `robolectric.offline`, `robolectric.dependency.dir`)
 */
class RobolectricConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        val catalog = target.extensions.getByType(VersionCatalogsExtension::class.java).named("libs")
        // One configuration per SDK: the artifacts share a module, and a single configuration
        // would conflict-resolve them to the highest version only.
        val jars =
            catalog.libraryAliases
                .filter { it.startsWith(ALIAS_PREFIX) }
                .map { alias ->
                    target.configurations.create("robolectricAndroidAll.$alias") {
                        isCanBeConsumed = false
                        isCanBeResolved = true
                        isTransitive = false
                        target.dependencies.add(name, catalog.findLibrary(alias).get())
                    }
                }

        val dir = target.layout.buildDirectory.dir("robolectric-android-all")
        val sync =
            target.tasks.register<Sync>("syncRobolectricAndroidAll") {
                from(jars)
                // Robolectric lists the directory contents when a jar is missing, so this file is
                // what a developer sees next to the "Unable to locate dependency" error.
                from(
                    target.resources.text
                        .fromString(
                            "Robolectric runs offline from this directory. A test that asks for an " +
                                "SDK with no jar here needs an entry robolectric-android-all-sdk<N> " +
                                "in apps/android/gradle/libs.versions.toml (see the comment there).\n"
                        )
                        .asFile()
                ) {
                    rename { "README.txt" }
                }
                into(dir)
            }
        target.tasks.withType<Test>().configureEach {
            val offline = OfflineArgs(target.objects.fileCollection(), target.objects.directoryProperty())
            offline.jars.from(sync)
            offline.dir.set(dir)
            jvmArgumentProviders.add(offline)
        }
    }

    /** The directory is an input (jars change when the catalog changes), its path is not. */
    class OfflineArgs(
        @get:InputFiles
        @get:PathSensitive(PathSensitivity.NONE)
        val jars: ConfigurableFileCollection,
        @get:Internal val dir: DirectoryProperty,
    ) : CommandLineArgumentProvider {
        override fun asArguments(): Iterable<String> =
            listOf("-Drobolectric.offline=true", "-Drobolectric.dependency.dir=${dir.get().asFile}")
    }

    private companion object {
        const val ALIAS_PREFIX = "robolectric.android.all."
    }
}
