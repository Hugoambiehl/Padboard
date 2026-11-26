// THROWAWAY PROTOTYPE (#20, step 1: screen mirroring over USB, no driver).
// A separate app (its own applicationId), debug-signed only, never shipped,
// never installed in place of Padboard. Lives on the proto/mirror branch.
plugins {
    alias(libs.plugins.android.application)
}

android {
    namespace = "io.github.hugoambiehl.padboard.mirrorproto"
    compileSdk {
        version = release(37)
    }
    defaultConfig {
        applicationId = "io.github.hugoambiehl.padboard.mirrorproto"
        minSdk = 24
        targetSdk = 37
        versionCode = 1
        versionName = "proto"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}
