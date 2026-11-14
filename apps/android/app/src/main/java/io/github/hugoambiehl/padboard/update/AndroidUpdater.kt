package io.github.hugoambiehl.padboard.update

import android.app.PendingIntent
import android.app.admin.DevicePolicyManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.os.Build
import io.github.hugoambiehl.padboard.connection.ApkChecks
import io.github.hugoambiehl.padboard.connection.AppUpdater
import io.github.hugoambiehl.padboard.protocol.UpdateOfferPayload
import io.github.hugoambiehl.padboard.protocol.UpdateOutcome
import java.io.File
import java.io.IOException
import java.security.MessageDigest
import java.util.logging.Logger
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/**
 * Updates sent by the PC, installed by the app itself: only as the device
 * owner (kiosk mode), which installs without asking and without the
 * `REQUEST_INSTALL_PACKAGES` permission (not declared). Before installing,
 * the APK must be this app, a higher version, and signed with this app's
 * certificate, and in a release build with the release key
 * ([ApkChecks.RELEASE_CERTIFICATE]). Android checks the signature again.
 */
class AndroidUpdater(private val context: Context) : AppUpdater {
    private val pm: PackageManager = context.packageManager
    private val own: PackageInfo = packageInfo(context.packageName)

    override val versionName: String? = own.versionName
    override val versionCode: Long = versionCodeOf(own)

    override fun canInstallSilently(): Boolean {
        val dpm = context.getSystemService(DevicePolicyManager::class.java) ?: return false
        return dpm.isDeviceOwnerApp(context.packageName)
    }

    override fun newApkFile(): File = File(context.cacheDir, APK_FILE).apply {
        delete()
        if (!createNewFile()) throw IOException("cannot create $path")
    }

    override fun check(apk: File, offer: UpdateOfferPayload): UpdateOutcome? {
        val archive = archiveInfo(apk)?.let { info ->
            ApkChecks.Apk(info.packageName, versionCodeOf(info), certificates(info))
        }
        val debuggable = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        val outcome = ApkChecks.check(
            archive,
            ownPackage = context.packageName,
            ownVersionCode = versionCode,
            ownCertificates = certificates(own),
            releaseCertificate = if (debuggable) null else ApkChecks.RELEASE_CERTIFICATE
        )
        if (outcome != null) log.warning("update ${offer.version} refused: $outcome")
        return outcome
    }

    override fun install(apk: File) {
        try {
            if (!canInstallSilently()) {
                InstallResults.report(UpdateOutcome.NotDeviceOwner)
                return
            }
            val installer = pm.packageInstaller
            val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                setAppPackageName(context.packageName)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
                }
            }
            val id = installer.createSession(params)
            installer.openSession(id).use { session ->
                apk.inputStream().use { input ->
                    session.openWrite("padboard.apk", 0, apk.length()).use { output ->
                        input.copyTo(output)
                        session.fsync(output)
                    }
                }
                // Explicit intent: the system adds the status extras (mutable on Android 12+).
                val intent = Intent(context, InstallStatusReceiver::class.java).setPackage(context.packageName)
                val mutable = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
                val status = PendingIntent.getBroadcast(
                    context,
                    id,
                    intent,
                    PendingIntent.FLAG_UPDATE_CURRENT or mutable
                )
                session.commit(status.intentSender)
            }
            log.info("update handed to the installer (session $id)")
        } catch (e: IOException) {
            log.warning("cannot install the update: $e")
            InstallResults.report(UpdateOutcome.InstallFailed)
        } catch (e: SecurityException) {
            log.warning("cannot install the update: $e")
            InstallResults.report(UpdateOutcome.InstallFailed)
        } finally {
            apk.delete()
        }
    }

    override val failures: SharedFlow<UpdateOutcome> = InstallResults.failures

    @Suppress("DEPRECATION")
    private fun packageInfo(name: String): PackageInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        pm.getPackageInfo(name, PackageManager.PackageInfoFlags.of(signingFlags().toLong()))
    } else {
        pm.getPackageInfo(name, signingFlags())
    }

    @Suppress("DEPRECATION")
    private fun archiveInfo(apk: File): PackageInfo? = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        pm.getPackageArchiveInfo(apk.path, PackageManager.PackageInfoFlags.of(signingFlags().toLong()))
    } else {
        pm.getPackageArchiveInfo(apk.path, signingFlags())
    }

    @Suppress("DEPRECATION")
    private fun signingFlags(): Int = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        PackageManager.GET_SIGNING_CERTIFICATES
    } else {
        PackageManager.GET_SIGNATURES
    }

    private companion object {
        const val APK_FILE = "update.apk"
        val log: Logger = Logger.getLogger(AndroidUpdater::class.java.name)

        @Suppress("DEPRECATION")
        fun versionCodeOf(info: PackageInfo): Long = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            info.longVersionCode
        } else {
            info.versionCode.toLong()
        }

        /** SHA-256 of the certificates the APK's contents are signed with. */
        @Suppress("DEPRECATION")
        fun certificates(info: PackageInfo): List<String> {
            val signatures =
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    info.signingInfo?.apkContentsSigners
                } else {
                    info.signatures
                }
            return signatures.orEmpty().map { signature ->
                MessageDigest.getInstance("SHA-256").digest(signature.toByteArray()).joinToString("") {
                    "%02x".format(it)
                }
            }
        }
    }
}

/** What the installer reports after the app handed it an update. */
object InstallResults {
    private val _failures =
        MutableSharedFlow<UpdateOutcome>(extraBufferCapacity = 4, onBufferOverflow = BufferOverflow.DROP_OLDEST)
    val failures: SharedFlow<UpdateOutcome> = _failures.asSharedFlow()

    fun report(outcome: UpdateOutcome) {
        _failures.tryEmit(outcome)
    }
}

/**
 * The installer's answer. Success replaces the app (`KioskUpdated` brings it
 * back in kiosk mode); a request for the user's confirmation means the app
 * is no longer the device owner: never shown, the update is refused.
 */
class InstallStatusReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
        when (status) {
            PackageInstaller.STATUS_SUCCESS -> log.info("update installed")
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                log.warning("the installer asks for a confirmation: not the device owner, refused")
                val sessionId = intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -1)
                if (sessionId != -1) runCatching { context.packageManager.packageInstaller.abandonSession(sessionId) }
                InstallResults.report(UpdateOutcome.NotDeviceOwner)
            }
            else -> {
                log.warning("update not installed: $status $message")
                InstallResults.report(UpdateOutcome.InstallFailed)
            }
        }
    }

    private companion object {
        val log: Logger = Logger.getLogger(InstallStatusReceiver::class.java.name)
    }
}
