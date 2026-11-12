package io.github.hugoambiehl.padboard.ui.about

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * What the tablet app is made of, for the "À propos et licences" screen:
 * Padboard itself, the fonts and icons it bundles, and the libraries listed
 * by Licensee at build time (`assets/licenses/libraries.json`, every license
 * checked against the allow-list in app/build.gradle.kts).
 */
data class Component(
    val name: String,
    /** Version, or the author for bundled assets. */
    val detail: String,
    val license: String,
    /** License texts, as files of `assets/licenses/`. */
    val texts: List<String>
)

/** Directory of the license texts and of Licensee's report in the assets. */
const val LICENSES_DIR = "licenses"
const val LIBRARIES_FILE = "libraries.json"

/** Full text bundled for each SPDX license the libraries use. */
private val TEXTS = mapOf("Apache-2.0" to "Apache-2.0.txt")

/** Libraries whose license carries their own copyright line. */
private val OWN_TEXTS = mapOf("androidx.datastore:datastore-preferences-external-protobuf" to "protobuf-LICENSE.txt")

val PADBOARD = Component("Padboard", "", "GPL-3.0-or-later", listOf("GPL-3.0.txt"))

/** Fonts and icons bundled as resources (not Gradle dependencies). */
val BUNDLED =
    listOf(
        Component("Inter", "The Inter Project Authors", "OFL-1.1", listOf("inter-OFL.txt")),
        Component("Space Grotesk", "The Space Grotesk Project Authors", "OFL-1.1", listOf("space-grotesk-OFL.txt")),
        Component("Lucide", "Lucide Icons and Contributors", "ISC", listOf("lucide-LICENSE.txt"))
    )

@Serializable
private data class Artifact(
    val groupId: String,
    val artifactId: String,
    val version: String,
    val name: String? = null,
    val spdxLicenses: List<SpdxLicense> = emptyList(),
    val unknownLicenses: List<UnknownLicense> = emptyList()
)

@Serializable
private data class SpdxLicense(val identifier: String)

@Serializable
private data class UnknownLicense(val name: String? = null, val url: String? = null)

private val json = Json { ignoreUnknownKeys = true }

/** Parses Licensee's `artifacts.json`, sorted by name. */
fun parseLibraries(text: String): List<Component> = json.decodeFromString<List<Artifact>>(text)
    .map { a ->
        val coordinates = "${a.groupId}:${a.artifactId}"
        val ids = a.spdxLicenses.map { it.identifier }
        Component(
            name = coordinates,
            detail = a.version,
            license = ids.joinToString(" / ").ifEmpty { a.unknownLicenses.firstNotNullOfOrNull { it.name } ?: "?" },
            texts = OWN_TEXTS[coordinates]?.let(::listOf) ?: ids.mapNotNull(TEXTS::get)
        )
    }.sortedBy { it.name }
