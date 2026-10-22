plugins { `kotlin-dsl` }

kotlin { jvmToolchain(21) }

dependencies {
    // The plugins themselves are on the root build's classpath (`apply false` in build.gradle.kts);
    // build-logic only needs their DSL types to compile.
    compileOnly(libs.android.gradle.plugin)
    compileOnly(libs.kotlin.gradle.plugin)
}

gradlePlugin {
    plugins {
        register("androidApplication") {
            id = "guardedsleep.android-application"
            implementationClass = "org.guardedsleep.buildlogic.AndroidApplicationConventionPlugin"
        }
        register("androidLibrary") {
            id = "guardedsleep.android-library"
            implementationClass = "org.guardedsleep.buildlogic.AndroidLibraryConventionPlugin"
        }
        register("compose") {
            id = "guardedsleep.compose"
            implementationClass = "org.guardedsleep.buildlogic.ComposeConventionPlugin"
        }
        register("i18n") {
            id = "guardedsleep.i18n"
            implementationClass = "org.guardedsleep.buildlogic.I18nConventionPlugin"
        }
        register("kotlin") {
            id = "guardedsleep.kotlin"
            implementationClass = "org.guardedsleep.buildlogic.KotlinConventionPlugin"
        }
    }
}
