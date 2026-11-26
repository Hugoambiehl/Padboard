pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        // Published on the Gradle Plugin Portal only: never asked to Maven
        // Central first. Central rate-limits GitHub's runners (429) and Gradle
        // then stops looking, reported as "Plugin … was not found" (#22).
        exclusiveContent {
            forRepository { gradlePluginPortal() }
            filter {
                includeGroupAndSubgroups("org.gradle.toolchains")
                includeGroupAndSubgroups("org.jlleitschuh.gradle")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "Padboard"
include(":app")
include(":mirrorproto")
