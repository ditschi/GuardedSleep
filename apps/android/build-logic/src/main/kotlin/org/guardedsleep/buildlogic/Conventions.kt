package org.guardedsleep.buildlogic

import com.android.build.api.dsl.ApplicationExtension
import com.android.build.api.dsl.CommonExtension
import com.android.build.api.dsl.LibraryExtension
import com.android.build.api.variant.ApplicationAndroidComponentsExtension
import java.io.StringReader
import java.util.Properties
import org.gradle.api.JavaVersion
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.gradle.kotlin.dsl.register
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.KotlinAndroidProjectExtension

internal const val COMPILE_SDK = 37
internal const val MIN_SDK = 26
internal const val TARGET_SDK = 36

/**
 * Directory with the host build of the core that the JVM tests load through JNA. Image runs build
 * into `$CARGO_TARGET_DIR` (`target/container`), native runs into `<repo>/target`; see
 * docs/contributing/development.md, "Rust execution mode".
 */
fun Project.coreHostLibDir(): String =
    "${System.getenv("CARGO_TARGET_DIR") ?: rootProject.file("../../target").path}/debug"

private fun CommonExtension.configureCommon() {
    compileSdk = COMPILE_SDK
    defaultConfig.minSdk = MIN_SDK
    compileOptions.sourceCompatibility = JavaVersion.VERSION_17
    compileOptions.targetCompatibility = JavaVersion.VERSION_17
    // Every finding is fixed or suppressed with a reason; there is no lint-baseline.xml.
    lint.warningsAsErrors = true
    lint.abortOnError = true
    // Dependency freshness is Renovate's job (weekly, release-age gated); these checks need the
    // network and would break a build the day a new release appears (AGP 9.5.1 did, 2026-11-06).
    lint.disable += setOf("AndroidGradlePluginVersion", "GradleDependency", "NewerVersionAvailable")
}

/** JDK 21 toolchain compiling to Java 17 bytecode, the same for every module. */
class KotlinConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.extensions.configure<KotlinAndroidProjectExtension> {
            jvmToolchain(21)
            compilerOptions.jvmTarget.set(JvmTarget.JVM_17)
        }
    }
}

class AndroidApplicationConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.pluginManager.apply("com.android.application")
        target.pluginManager.apply("guardedsleep.kotlin")
        target.extensions.configure<ApplicationExtension> {
            configureCommon()
            defaultConfig.targetSdk = TARGET_SDK
            // version.properties is the only place a release number is written.
            val versionName =
                target.providers
                    .fileContents(
                        target.rootProject.layout.projectDirectory.file("version.properties")
                    )
                    .asText
                    .map { text ->
                        Properties().apply { load(StringReader(text)) }.getProperty("versionName")
                            ?: error("version.properties has no versionName")
                    }
                    .get()
            defaultConfig.versionName = versionName
            defaultConfig.versionCode = AndroidVersion.versionCode(versionName)
        }
        configureLocaleFilters(target)
    }
}

/**
 * `androidResources.localeFilters` is generated from the shipped locales (see
 * [ShippedLocalesTask]), so the APK lists exactly the generated languages. Debug builds also keep
 * the platform pseudo-locales (en-XA accented, ar-XB right-to-left) that `isPseudoLocalesEnabled`
 * adds; a filter would otherwise drop them. Doc:
 * https://developer.android.com/reference/tools/gradle-api/9.4/com/android/build/api/variant/ApplicationAndroidResources
 */
private fun configureLocaleFilters(target: Project) {
    val root = target.rootProject.layout.projectDirectory.dir("../..")
    val shipped =
        target.tasks.register<ShippedLocalesTask>("shippedLocales") {
            repoRoot.set(root)
            localesDir.set(root.dir("locales"))
            generatorDir.set(root.dir("crates/i18n-gen/src"))
            generatorManifests.from(
                root.file("crates/i18n-gen/Cargo.toml"),
                root.file("Cargo.lock"),
            )
            outputFile.set(target.layout.buildDirectory.file("generated/i18n/locales.txt"))
        }
    val tags =
        shipped
            .flatMap { it.outputFile }
            .map { file ->
                // BCP 47 tags with a script subtag (zh-Hans) need AAPT's `b+zh+Hans` form; plain
                // language tags pass through. Doc:
                // https://developer.android.com/guide/topics/resources/multilingual-support
                file.asFile
                    .readLines()
                    .filter { it.isNotBlank() }
                    .map { tag ->
                        if ('-' in tag) "b+" + tag.replace('-', '+') else tag
                    }
            }
    target.extensions.configure<ApplicationAndroidComponentsExtension> {
        onVariants { variant ->
            variant.androidResources.localeFilters.addAll(tags)
            if (variant.debuggable) {
                variant.androidResources.localeFilters.addAll(listOf("en-rXA", "ar-rXB"))
            }
        }
    }
}

class AndroidLibraryConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.pluginManager.apply("com.android.library")
        target.pluginManager.apply("guardedsleep.kotlin")
        target.extensions.configure<LibraryExtension> {
            configureCommon()
            // R8 keep rules a library needs travel with it: see core-bindings and platform.
            if (target.file("consumer-rules.pro").exists()) {
                defaultConfig.consumerProguardFiles("consumer-rules.pro")
            }
        }
    }
}

class ComposeConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.pluginManager.apply("org.jetbrains.kotlin.plugin.compose")
        target.extensions.configure<CommonExtension> { buildFeatures.compose = true }
    }
}
