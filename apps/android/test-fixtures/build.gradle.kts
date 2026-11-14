// Test doubles and the two-phone harness for the JVM tests (`ui-journey-tests`): a virtual-time
// scheduler, an in-memory signaling relay, fake media and system services, and `TestPhones`, which
// wires two real LinkControllers (with the real core) together. Never packaged into an app.
//
// Boundary: this module sees `core-bindings` (the fakes implement ports that carry UniFFI types),
// but its public API for the UI tests returns only `AppController` and contract types, so
// `app/src/test` never imports `uniffi.*`.
plugins {
    id("guardedsleep.android-library")
}

android {
    namespace = "org.guardedsleep.app.fixtures"

    // Notification and settings classes are used under Robolectric or plain JUnit by the callers.
    testOptions.unitTests.isIncludeAndroidResources = true
}

dependencies {
    api(project(":ui-contract"))
    implementation(project(":platform"))
    implementation(project(":core-bindings"))
    implementation(libs.datastore.preferences)
    implementation(libs.coroutines.android)
}
