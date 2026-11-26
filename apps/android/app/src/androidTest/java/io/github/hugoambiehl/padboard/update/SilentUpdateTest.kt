package io.github.hugoambiehl.padboard.update

import android.os.Build
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.hugoambiehl.padboard.protocol.UpdateOfferPayload
import java.io.File
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The silent update of a kiosk tablet (device owner) on this device's
 * Android, as an update over Wi-Fi ends: [AndroidUpdater] checks the APK
 * (package, versionCode, signing certificates read from the archive), then
 * commits a `PackageInstaller` session without any confirmation.
 *
 * Opt-in and destructive: the app must be the device owner, and
 * `-e padboardUpdateApk <name>` names a newer build of the same app (same
 * key, higher versionCode, e.g. `-Ppadboard.testVersionCode=…`) copied into
 * the app's cache folder (`run-as`). Success replaces the app, which ends
 * this process: the instrumentation then reports it lost, and the result is
 * read afterwards (`dumpsys package`: the new versionCode; the app back in
 * front, still locked). Still running after [WAIT_MS]: the update failed.
 */
@RunWith(AndroidJUnit4::class)
class SilentUpdateTest {
    private val name: String? = InstrumentationRegistry.getArguments().getString("padboardUpdateApk")

    @Test
    fun installsANewerVersionSilently() {
        assumeTrue("no APK given (-e padboardUpdateApk)", name != null)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val updater = AndroidUpdater(context)
        assertTrue("the app must be the device owner", updater.canInstallSilently())

        val apk = File(context.cacheDir, name!!)
        assertTrue("$apk is missing", apk.isFile)
        val offer = UpdateOfferPayload(version = "test", versionCode = 0u, size = apk.length().toUInt(), sha256 = "")
        assertNull("the APK must pass the app's checks", updater.check(apk, offer))

        Log.i(TAG, "API ${Build.VERSION.SDK_INT}: installing ${apk.name} over versionCode ${updater.versionCode}")
        updater.install(apk)
        Thread.sleep(WAIT_MS)
        throw AssertionError("still running ${WAIT_MS / 1000} s later: the update was not installed (see logcat)")
    }

    private companion object {
        const val TAG = "SilentUpdateTest"
        const val WAIT_MS = 60_000L
    }
}
