package io.github.hugoambiehl.padboard.ui.about

import org.junit.Assert.assertEquals
import org.junit.Test

class LicensesTest {
    @Test
    fun `libraries are sorted and point to their license texts`() {
        val json =
            """
            [
              {"groupId": "com.squareup.okhttp3", "artifactId": "okhttp", "version": "5.5.1",
               "spdxLicenses": [{"identifier": "Apache-2.0", "name": "Apache License 2.0", "url": "x"}],
               "scm": {"url": "https://github.com/square/okhttp"}},
              {"groupId": "androidx.datastore", "artifactId": "datastore-preferences-external-protobuf",
               "version": "1.3.0", "spdxLicenses": [{"identifier": "BSD-3-Clause"}]},
              {"groupId": "org.example", "artifactId": "odd", "version": "1",
               "unknownLicenses": [{"name": "Custom", "url": "https://example.org"}]}
            ]
            """.trimIndent()

        val libraries = parseLibraries(json)

        assertEquals(
            listOf(
                Component(
                    "androidx.datastore:datastore-preferences-external-protobuf",
                    "1.3.0",
                    "BSD-3-Clause",
                    listOf("protobuf-LICENSE.txt")
                ),
                Component("com.squareup.okhttp3:okhttp", "5.5.1", "Apache-2.0", listOf("Apache-2.0.txt")),
                Component("org.example:odd", "1", "Custom", emptyList())
            ),
            libraries
        )
    }
}
