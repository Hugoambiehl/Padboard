package io.github.hugoambiehl.padboard.kiosk

import android.app.Activity
import android.app.ActivityManager
import android.app.Application
import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.util.Log
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStoreOwner
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry
import androidx.test.runner.lifecycle.Stage
import io.github.hugoambiehl.padboard.MainActivity
import io.github.hugoambiehl.padboard.protocol.KioskExitOrigin
import io.github.hugoambiehl.padboard.ui.PadboardViewModel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeFalse
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Kiosk mode turned on by the PC, the screen put to sleep and woken up,
 * kiosk mode left and turned on again: the app keeps one screen and one
 * connection to the PC throughout.
 *
 * On the SM-T580 (Android 12L) the PC's `am start` left the activity in a
 * standard task, and the system started the home entry (`HomeAlias`) in a
 * home task at the first wake-up: two activities, two connections of the
 * same tablet replacing each other on the PC twice a second. The emulators
 * do not start the home app when the screen wakes up, so the test sends the
 * home intent itself, as the system does then. Android 12L (API 32) keeps
 * both activities; Android 7 reuses the first one, Android 17 drops it.
 *
 * Sets the device owner with `dpm` (an emulator without accounts) and
 * leaves kiosk mode at the end, which gives the role up. Run by
 * `scripts/tablet-ui-tests.sh`; it fails without the fix on API 32
 * (`PADBOARD_TEST_IMAGE="system-images;android-32;google_apis;x86_64"`)
 * and guards #24 on API 24.
 */
@RunWith(AndroidJUnit4::class)
class KioskSingleInstanceTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val app = instrumentation.targetContext.applicationContext as Application
    private val pkg = app.packageName

    @After
    fun leaveKiosk() {
        onMain { Kiosk.exit(app, KioskExitOrigin.Pc) }
    }

    @Test
    fun oneScreenAndOneConnectionThroughKioskSleepAndExit() {
        // The API 37 emulator images (Play and AOSP, any -gpu) abort
        // system_server when they snapshot a task switch ("Assertion failed:
        // !rcEnc->featureInfo()->hasReadColorBufferDma"), which this test does.
        assumeFalse(
            "API 37 emulator: task switches crash the system",
            Build.VERSION.SDK_INT >= 37 && Build.HARDWARE == "ranchu"
        )
        // The app opened like any other one, before kiosk mode.
        shell("am start -W -n $pkg/.MainActivity")
        waitFor("the app in front") { resumed() != null }
        for (round in 1..2) {
            enableKioskLikeThePc()
            check("kiosk mode turned on (round $round)", kiosk = true)
            sleepAndWake()
            // What Android 12L does at the first wake-up in kiosk mode.
            shell("am start -W -a android.intent.action.MAIN -c android.intent.category.HOME")
            check("woken up (round $round)", kiosk = true)
            onMain { Kiosk.exit(app, KioskExitOrigin.Pc) }
            // #24: still in front, as a normal app.
            check("kiosk mode left (round $round)", kiosk = false)
        }
    }

    /**
     * #32: "Libérer la tablette" while the tablet's screen is off. No screen
     * of the app is in front then; at wake-up it must be, as a normal app.
     */
    @Test
    fun leavingFromThePcWhileTheScreenIsOffKeepsTheAppInFront() {
        assumeFalse(
            "API 37 emulator: task switches crash the system",
            Build.VERSION.SDK_INT >= 37 && Build.HARDWARE == "ranchu"
        )
        shell("am start -W -n $pkg/.MainActivity")
        waitFor("the app in front") { resumed() != null }
        enableKioskLikeThePc()
        check("kiosk mode turned on", kiosk = true)
        // The PC's order arrives 1.5 s into a 4 s sleep (one shell command:
        // a sleeping emulator may suspend before a second one).
        var stageAtExit: Stage? = null
        val exit =
            Thread {
                SystemClock.sleep(1_500)
                instrumentation.runOnMainSync {
                    val monitor = ActivityLifecycleMonitorRegistry.getInstance()
                    stageAtExit =
                        Stage.entries
                            .firstOrNull { stage -> monitor.getActivitiesInStage(stage).any { it is MainActivity } }
                    Kiosk.exit(app, KioskExitOrigin.Pc)
                }
            }
        exit.start()
        shell("input keyevent KEYCODE_SLEEP && sleep 4 && input keyevent KEYCODE_WAKEUP")
        exit.join()
        Log.i(TAG, "screen off: the app's screen was $stageAtExit when the order came")
        check("woken up after leaving kiosk mode", kiosk = false)
    }

    /** What the PC's "Activer" does (`adb.rs`, `enable_kiosk`). */
    private fun enableKioskLikeThePc() {
        val owner = "$pkg/.kiosk.KioskAdmin"
        // Right after boot, the account services may still refuse it for a while.
        waitFor("the device owner set", timeoutMs = 120_000) {
            shell("dpm set-device-owner $owner").contains("Success") || owns()
        }
        shell("am start -W -n $pkg/.MainActivity")
        waitFor("kiosk mode on") { Kiosk.active.value }
    }

    private fun sleepAndWake() {
        // In one command: a sleeping emulator may suspend before the next one.
        shell("input keyevent KEYCODE_SLEEP && sleep 1 && input keyevent KEYCODE_WAKEUP")
        SystemClock.sleep(2_000)
    }

    /** One activity in front, one connection; locked in kiosk mode only. */
    private fun check(step: String, kiosk: Boolean) {
        waitFor("$step: the app in front") { resumed() != null }
        // Leave time to a second instance, or to the end of an older one.
        SystemClock.sleep(3_000)
        val live = live()
        Log.i(
            TAG,
            "$step: ${live.map {
                it.componentName.shortClassName
            }}, ${shell("dumpsys activity activities").lines().filter { "LockTaskMode" in it }}"
        )
        assertEquals("$step: activities of the app", 1, live.size)
        assertTrue("$step: the app in front", resumed() != null)
        val links =
            onMain {
                live.map { ViewModelProvider(it as ViewModelStoreOwner)[PadboardViewModel::class.java].link }.toSet()
            }
        assertEquals("$step: connections to the PC", 1, links.size)
        assertEquals("$step: device owner", kiosk, owns())
        val locked =
            app.getSystemService(ActivityManager::class.java).lockTaskModeState != ActivityManager.LOCK_TASK_MODE_NONE
        assertEquals("$step: locked in", kiosk, locked)
        if (!kiosk) {
            assertFalse(Kiosk.active.value)
            // A normal tablet never asks which home app to use.
            val home = app.packageManager.getComponentEnabledSetting(ComponentName(app, "$pkg.HomeAlias"))
            assertEquals("$step: home entry", PackageManager.COMPONENT_ENABLED_STATE_DISABLED, home)
        }
    }

    private fun owns() = app.getSystemService(android.app.admin.DevicePolicyManager::class.java).isDeviceOwnerApp(pkg)

    private fun live(): List<Activity> = onMain {
        val monitor = ActivityLifecycleMonitorRegistry.getInstance()
        listOf(Stage.CREATED, Stage.STARTED, Stage.RESUMED, Stage.PAUSED, Stage.STOPPED, Stage.RESTARTED)
            .flatMap { monitor.getActivitiesInStage(it) }
            .filter { it is MainActivity && !it.isFinishing }
            .distinct()
    }

    private fun resumed(): Activity? {
        dismissLocalNetworkPicker()
        return resumedNow()
    }

    /**
     * Android 17 asks which device of the local network the app may reach
     * when it looks for PCs (mDNS): not what this test is about, so the
     * question is turned down ("Don't connect").
     */
    private fun dismissLocalNetworkPicker() {
        if (Build.VERSION.SDK_INT < 37) return
        val top = shell("dumpsys activity activities").lines().firstOrNull { "topResumedActivity=" in it }.orEmpty()
        if ("NsdPickerActivity" in top) shell("input keyevent KEYCODE_BACK")
    }

    private fun resumedNow(): Activity? = onMain {
        ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED).firstOrNull {
            it is MainActivity
        }
    }

    private fun <T> onMain(block: () -> T): T {
        var result: Result<T>? = null
        instrumentation.runOnMainSync { result = runCatching(block) }
        return result!!.getOrThrow()
    }

    private fun waitFor(what: String, timeoutMs: Long = 20_000, condition: () -> Boolean) {
        val deadline = SystemClock.uptimeMillis() + timeoutMs
        while (!condition()) {
            assertTrue("timed out waiting for $what", SystemClock.uptimeMillis() < deadline)
            SystemClock.sleep(250)
        }
    }

    private fun shell(command: String): String {
        val fd = instrumentation.uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(fd).bufferedReader().use { it.readText() }
    }

    private companion object {
        const val TAG = "KioskSingleInstanceTest"
    }
}
