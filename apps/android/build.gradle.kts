plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
    // AGP 9 compiles Kotlin itself; this only pins the Kotlin Gradle plugin version it uses.
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.roborazzi) apply false
}

// Unit tests of the convention plugins (versionCode ordering); runs in the same Gradle invocation
// as the module tests.
tasks.register("buildLogicTest") { dependsOn(gradle.includedBuild("build-logic").task(":test")) }
