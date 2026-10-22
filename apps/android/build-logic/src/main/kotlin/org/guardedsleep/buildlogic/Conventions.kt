package org.guardedsleep.buildlogic

import com.android.build.api.dsl.ApplicationExtension
import com.android.build.api.dsl.CommonExtension
import com.android.build.api.dsl.LibraryExtension
import org.gradle.api.JavaVersion
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.KotlinAndroidProjectExtension

internal const val COMPILE_SDK = 37
internal const val MIN_SDK = 26
internal const val TARGET_SDK = 36

private fun CommonExtension.configureCommon() {
    compileSdk = COMPILE_SDK
    defaultConfig.minSdk = MIN_SDK
    compileOptions.sourceCompatibility = JavaVersion.VERSION_17
    compileOptions.targetCompatibility = JavaVersion.VERSION_17
    // Every finding is fixed or suppressed with a reason; there is no lint-baseline.xml.
    lint.warningsAsErrors = true
    lint.abortOnError = true
    // Dependency freshness is Renovate's job (weekly, release-age gated); these two checks need
    // the network and would break a build the day a new release appears.
    lint.disable += setOf("GradleDependency", "NewerVersionAvailable")
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
        }
    }
}

class AndroidLibraryConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.pluginManager.apply("com.android.library")
        target.pluginManager.apply("guardedsleep.kotlin")
        target.extensions.configure<LibraryExtension> { configureCommon() }
    }
}

class ComposeConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) {
        target.pluginManager.apply("org.jetbrains.kotlin.plugin.compose")
        target.extensions.configure<CommonExtension> { buildFeatures.compose = true }
    }
}
