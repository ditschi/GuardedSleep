// UI (Compose screens), navigation and the manual dependency wiring. Depends on `ui-contract` and
// `platform` only: UniFFI types are not on this module's compile classpath.
plugins {
    id("guardedsleep.android-application")
    id("guardedsleep.compose")
    alias(libs.plugins.roborazzi)
}

android {
    namespace = "org.guardedsleep.app"

    defaultConfig {
        // ADR-018: alpha builds install next to the Capacitor app (org.guardedsleep.app).
        applicationId = "org.guardedsleep.app.alpha"
        // Must match the ABIs scripts/core-build.sh builds; libwebrtc alone would ship more,
        // and the app would then crash on devices without a matching core library.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        // Commit identity comes from scripts/gradle.sh (host git), never from the build time, so
        // builds stay reproducible.
        fun gitField(name: String, env: String) =
            buildConfigField(
                "String",
                name,
                "\"${providers.environmentVariable(env).orNull?.takeIf { it.isNotBlank() } ?: "unknown"}\"",
            )
        gitField("GIT_COMMIT", "GS_GIT_COMMIT")
        gitField("GIT_COMMIT_TIME", "GS_GIT_COMMIT_TIME")
    }

    // BuildConfig carries the commit shown in Settings. Doc:
    // https://developer.android.com/build/gradle-tips#share-custom-fields-and-resource-values-with-your-app-code
    buildFeatures { buildConfig = true }

    buildTypes {
        debug {
            isPseudoLocalesEnabled = true
            // Emulator gate and local dev signaling use ws:// on the emulator network only.
            manifestPlaceholders["usesCleartextTraffic"] = "true"
        }
        release {
            // R8 shrinks and obfuscates; the keep rules for UniFFI/JNA and libwebrtc are the
            // consumer-rules.pro of core-bindings and platform. Doc:
            // https://developer.android.com/build/shrink-code
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
            manifestPlaceholders["usesCleartextTraffic"] = "false"
        }
    }

    // AppLocale can switch to any shipped language at runtime, so the bundle must not split
    // resources by device language. Doc:
    // https://developer.android.com/guide/app-bundle/configure-base#handle_language_changes
    bundle { language { enableSplit = false } }

    // Store native libraries compressed in the APK (about 40 % of their size); the installer
    // extracts them, so the installed size grows by the library size. AGP 8+ defaults to
    // uncompressed (page-aligned, mapped from the APK). Doc:
    // https://developer.android.com/build/releases/past-releases/agp-4-2-0-release-notes#compress-native-libs-dsl
    packaging { jniLibs { useLegacyPackaging = true } }

    testOptions { unitTests.isIncludeAndroidResources = true }
}

// APK files are named guardedsleep-<variant>.apk instead of app-<variant>.apk. AGP takes the file
// name from the Gradle base plugin's archivesName. Doc:
// https://docs.gradle.org/current/dsl/org.gradle.api.plugins.BasePluginExtension.html
base { archivesName = "guardedsleep" }

dependencies {
    implementation(project(":ui-contract"))
    implementation(project(":platform"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.camera.camera2)
    implementation(libs.camera.lifecycle)
    implementation(libs.camera.view)
    implementation(libs.zxing.core)
    implementation(libs.zxing.cpp)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.datastore.preferences)
    implementation(libs.coroutines.android)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(libs.roborazzi.junit.rule)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.androidx.test.core)
    testImplementation(libs.androidx.test.ext.junit)
    testImplementation(libs.coroutines.test)
    debugImplementation(libs.compose.ui.test.manifest)
}
