import javax.inject.Inject

plugins {
    id("guardedsleep.android-application")
    id("guardedsleep.compose")
    alias(libs.plugins.roborazzi)
}

// Android string resources are generated from the platform-neutral catalogue (locales/*.json).
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

val generateI18n =
    tasks.register<GenerateI18nTask>("generateI18n") {
        val root = rootProject.layout.projectDirectory.dir("../..")
        repoRoot.set(root)
        localesDir.set(root.dir("locales"))
        generatorDir.set(root.dir("crates/i18n-gen/src"))
        outputDir.set(layout.buildDirectory.dir("generated/i18n/res"))
    }

androidComponents {
    onVariants { variant ->
        variant.sources.res?.addGeneratedSourceDirectory(generateI18n, GenerateI18nTask::outputDir)
    }
}

android {
    namespace = "org.guardedsleep.app"
    defaultConfig {
        // ADR-018: alpha builds install next to the Capacitor app (org.guardedsleep.app).
        applicationId = "org.guardedsleep.app.alpha"
        versionCode = 1
        versionName = "0.1.0-alpha.1"
        // Must match the ABIs scripts/core-build.sh builds; libwebrtc alone would ship more,
        // and the app would then crash on devices without a matching core library.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildTypes {
        debug {
            isPseudoLocalesEnabled = true
            // Emulator gate and local dev signaling use ws:// on the emulator network only.
            manifestPlaceholders["usesCleartextTraffic"] = "true"
        }
        release {
            isMinifyEnabled = false
            manifestPlaceholders["usesCleartextTraffic"] = "false"
        }
    }

    buildFeatures {
        buildConfig = true
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
        unitTests.all {
            // Host build of the core (scripts/core-build.sh android); JNA loads it for the real
            // CoreLink in tests.
            it.systemProperty(
                "jna.library.path",
                rootProject.file("../../target/debug").absolutePath,
            )
        }
    }
}

dependencies {
    implementation(project(":core"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.webrtc.sdk)
    implementation(libs.okhttp)
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
    testImplementation(libs.jna)
    testImplementation(libs.json)
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
