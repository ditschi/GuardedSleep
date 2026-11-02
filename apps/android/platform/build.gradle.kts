// Everything that touches the OS or the network: monitor service, alerts, signaling, libwebrtc
// engine, pairing storage. The only module that depends on `core-bindings` (UniFFI).
plugins {
    id("guardedsleep.android-library")
    // PublicLists.kt: the curated relay and STUN servers, generated from data/*.json.
    id("guardedsleep.public-lists")
}

android {
    namespace = "org.guardedsleep.app.platform"

    buildFeatures { buildConfig = true }

    testOptions {
        // Notification text tests (Robolectric) read the contract module's string resources.
        unitTests.isIncludeAndroidResources = true
        unitTests.all {
            // Host build of the core (scripts/core-build.sh host); JNA loads it for the real
            // CoreLink in tests. AGP drops .so files from unit-test resources.
            it.systemProperty(
                "jna.library.path",
                rootProject.file("../../target/debug").absolutePath,
            )
        }
    }
}

dependencies {
    implementation(project(":ui-contract"))
    implementation(project(":core-bindings"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.webrtc.sdk)
    implementation(libs.okhttp)
    implementation(libs.datastore.preferences)
    implementation(libs.coroutines.android)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)
    testImplementation(libs.jna)
    testImplementation(libs.json)
    testImplementation(libs.coroutines.test)
}
