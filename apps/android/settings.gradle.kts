pluginManagement {
    includeBuild("build-logic")
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // Repository-confusion guard: Google Maven serves only Google/AndroidX groups, and the
        // libwebrtc group resolves from Maven Central only. No JitPack.
        google {
            content {
                includeGroupByRegex("com\\.android(\\..*)?")
                includeGroupByRegex("com\\.google\\..*")
                includeGroupByRegex("androidx\\..*")
            }
        }
        exclusiveContent {
            forRepository { mavenCentral() }
            filter { includeGroup("io.github.webrtc-sdk") }
        }
        mavenCentral()
    }
}

rootProject.name = "guardedsleep-native"

include(":app", ":ui-contract", ":platform", ":core-bindings", ":test-fixtures")
