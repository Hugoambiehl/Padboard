package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.UpdateOutcome
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ApkChecksTest {
    private val own = "io.github.hugoambiehl.padboard"
    private val release = ApkChecks.RELEASE_CERTIFICATE
    private val debugKey = "aa".repeat(32)

    private fun apk(pkg: String = own, code: Long = 30099, certs: List<String> = listOf(release)) =
        ApkChecks.Apk(pkg, code, certs)

    private fun check(apk: ApkChecks.Apk?, ownCerts: List<String> = listOf(release), releaseBuild: Boolean = true) =
        ApkChecks.check(apk, own, 20099, ownCerts, if (releaseBuild) release else null)

    @Test
    fun aNewerReleaseOfThisAppIsAccepted() {
        assertNull(check(apk()))
        // Certificates compare whatever their case.
        assertNull(check(apk(certs = listOf(release.uppercase()))))
    }

    @Test
    fun anotherAppOrAnOlderVersionIsRefused() {
        assertEquals(UpdateOutcome.WrongPackage, check(apk(pkg = "com.example.evil")))
        assertEquals(UpdateOutcome.NotNewer, check(apk(code = 20099)))
        assertEquals(UpdateOutcome.NotNewer, check(apk(code = 10099)))
    }

    @Test
    fun anotherKeyIsRefused() {
        assertEquals(UpdateOutcome.BadCertificate, check(apk(certs = listOf(debugKey))))
        assertEquals(UpdateOutcome.BadCertificate, check(apk(certs = emptyList())))
        // Signed with the right key and another one: not exactly this app's.
        assertEquals(UpdateOutcome.BadCertificate, check(apk(certs = listOf(release, debugKey))))
        // Unreadable: nothing can be checked.
        assertEquals(UpdateOutcome.BadCertificate, check(null))
    }

    @Test
    fun aReleaseBuildOnlyTakesTheReleaseKey() {
        // A debug build updates from builds with its own (debug) key...
        assertNull(check(apk(certs = listOf(debugKey)), ownCerts = listOf(debugKey), releaseBuild = false))
        // ...a release build never, even if it were itself signed with it.
        assertEquals(
            UpdateOutcome.BadCertificate,
            check(apk(certs = listOf(debugKey)), ownCerts = listOf(debugKey), releaseBuild = true)
        )
    }
}
