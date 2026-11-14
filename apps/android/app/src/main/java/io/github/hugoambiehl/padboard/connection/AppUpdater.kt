package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.UpdateOfferPayload
import io.github.hugoambiehl.padboard.protocol.UpdateOutcome
import java.io.File
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow

/**
 * The tablet app's own version, and how an update offered by the PC is
 * checked and installed (Android side: `update/AndroidUpdater.kt`). Kept
 * out of [ConnectionManager] so the protocol stays testable on the JVM.
 */
interface AppUpdater {
    /** `versionName`, sent in `hello`. */
    val versionName: String?

    /** `versionCode`: an update must be above it. */
    val versionCode: Long?

    /** Device owner (kiosk mode): an update installs without asking. Only then is one accepted. */
    fun canInstallSilently(): Boolean

    /** An empty file to receive an APK into. */
    fun newApkFile(): File

    /**
     * Checks a received APK (package, version above this one, signing
     * certificate): null when it may be installed. Blocking.
     */
    fun check(apk: File, offer: UpdateOfferPayload): UpdateOutcome?

    /** Hands the checked APK to Android's installer and deletes it. The app is replaced, or [failures] tells why not. Blocking. */
    fun install(apk: File)

    /** Installs that failed after [install] returned. */
    val failures: SharedFlow<UpdateOutcome>

    /** No updates (tests that do not need them). */
    object None : AppUpdater {
        override val versionName: String? = null
        override val versionCode: Long? = null

        override fun canInstallSilently() = false

        override fun newApkFile(): File = error("no updates")

        override fun check(apk: File, offer: UpdateOfferPayload): UpdateOutcome = UpdateOutcome.NotDeviceOwner

        override fun install(apk: File) = Unit

        override val failures: SharedFlow<UpdateOutcome> = MutableSharedFlow()
    }
}

/** Where an update over the connection is, for the UI. */
sealed interface UpdateProgress {
    val version: String

    data class Receiving(override val version: String, val received: Long, val total: Long) : UpdateProgress

    /** Checked and handed to the installer: the app restarts in the new version. */
    data class Installing(override val version: String) : UpdateProgress

    data class Failed(override val version: String, val outcome: UpdateOutcome) : UpdateProgress
}

/** The checks on a received APK, once read (pure, tested on the JVM). */
object ApkChecks {
    /** What the APK says about itself; certificates as lowercase SHA-256 hex. */
    data class Apk(val packageName: String, val versionCode: Long, val certificates: List<String>)

    /**
     * Null when [apk] may replace this app: same package, a higher
     * version, signed with exactly this app's certificates and, in a
     * release build ([releaseCertificate] set), with the release key only.
     * An unreadable APK ([apk] null) cannot be trusted.
     */
    fun check(
        apk: Apk?,
        ownPackage: String,
        ownVersionCode: Long,
        ownCertificates: List<String>,
        releaseCertificate: String?
    ): UpdateOutcome? {
        if (apk == null) return UpdateOutcome.BadCertificate
        if (apk.packageName != ownPackage) return UpdateOutcome.WrongPackage
        if (apk.versionCode <= ownVersionCode) return UpdateOutcome.NotNewer
        val signers = apk.certificates.map { it.lowercase() }.toSet()
        if (signers.isEmpty() || signers != ownCertificates.map { it.lowercase() }.toSet()) {
            return UpdateOutcome.BadCertificate
        }
        if (releaseCertificate != null && signers != setOf(releaseCertificate.lowercase())) {
            return UpdateOutcome.BadCertificate
        }
        return null
    }

    /** SHA-256 of the release key's certificate (docs/release-signing.md). */
    const val RELEASE_CERTIFICATE = "637bb3c07a1bc2a0c10d92f4152ef9c92fbf00c9d510b3e72f63f7c2e3dcfa46"
}
