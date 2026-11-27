package io.github.hugoambiehl.padboard.connection

import android.Manifest
import android.app.Application
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.hugoambiehl.padboard.kiosk.Kiosk
import io.github.hugoambiehl.padboard.protocol.KioskExitOrigin
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters

/**
 * Android 17 (API 37) and the local network permission (#31). Without it,
 * looking for PCs over mDNS made the system open its device picker in front
 * of the app at every launch, kiosk mode included. Now the app asks for the
 * permission only when Wi-Fi is wanted, and in kiosk mode grants it itself.
 *
 * Skipped below API 37, and run on demand (see android17()), alone:
 *
 *     PADBOARD_TEST_GPU=swiftshader_indirect PADBOARD_TEST_RAM=4096 \
 *     PADBOARD_TEST_ARGS="-e class io.github.hugoambiehl.padboard.connection.LocalNetworkTest -e padboardLocalNetwork true" \
 *     scripts/tablet-ui-tests.sh
 *
 * In name order: the first test needs the permission not granted yet (a
 * fresh emulator), the second grants it.
 */
@RunWith(AndroidJUnit4::class)
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class LocalNetworkTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val app = instrumentation.targetContext.applicationContext as Application
    private val pkg = app.packageName

    @Before
    fun android17() {
        assumeTrue("Android 17 and later only", Build.VERSION.SDK_INT >= 37)
        // The API 37 emulator aborts system_server now and then on a task
        // switch, which this test makes (kiosk mode starts the home entry): run
        // on demand, alone, so the other tests are not taken down with it.
        assumeTrue(
            "on demand: -e padboardLocalNetwork true",
            InstrumentationRegistry.getArguments().getString("padboardLocalNetwork") == "true"
        )
    }

    @Test
    fun a_noSystemPickerAtLaunch() {
        assumeTrue("permission already granted (not a fresh install)", !granted())
        shell("am start -W -n $pkg/.MainActivity")
        // The picker came within a second or two of the launch.
        SystemClock.sleep(6_000)
        val top = shell("dumpsys activity activities").lines().firstOrNull { "topResumedActivity=" in it }.orEmpty()
        assertFalse("the system's local network picker is in front: $top", "NsdPickerActivity" in top)
        assertEquals("the app in front", true, pkg in top)
    }

    @Test
    fun b_kioskModeGrantsItself() {
        val owner = "$pkg/.kiosk.KioskAdmin"
        val deadline = SystemClock.uptimeMillis() + 120_000
        while (!shell("dpm set-device-owner $owner").contains("Success")) {
            check(SystemClock.uptimeMillis() < deadline) { "the device owner could not be set" }
            SystemClock.sleep(3_000)
        }
        try {
            instrumentation.runOnMainSync { Kiosk.refresh(app) }
            assertEquals("granted by the device owner", true, granted())
            assertEquals("seen by the app", true, LocalNetwork.allowed.value)
        } finally {
            instrumentation.runOnMainSync { Kiosk.exit(app, KioskExitOrigin.Pc) }
        }
    }

    private fun granted() =
        app.checkSelfPermission(Manifest.permission.ACCESS_LOCAL_NETWORK) == PackageManager.PERMISSION_GRANTED

    private fun shell(command: String): String {
        val fd = instrumentation.uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(fd).bufferedReader().use { it.readText() }
    }
}
