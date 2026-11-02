// State models and actions the UI renders (UiContract.kt) plus the generated string catalogue.
// No UniFFI, no Compose: this module is the only thing `app` and `platform` share.
plugins {
    id("guardedsleep.android-library")
    id("guardedsleep.i18n")
}

android { namespace = "org.guardedsleep.app.contract" }

dependencies { api(libs.coroutines.android) }
