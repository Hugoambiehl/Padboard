package io.github.hugoambiehl.padboard.kiosk

import android.app.admin.DeviceAdminReceiver
import android.app.admin.DevicePolicyManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.edit
import io.github.hugoambiehl.padboard.connection.KioskExitLog
import io.github.hugoambiehl.padboard.connection.LocalNetwork
import io.github.hugoambiehl.padboard.protocol.KioskExitOrigin
import io.github.hugoambiehl.padboard.protocol.KioskLeft
import java.util.logging.Logger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Kiosk mode (see docs/install-tablet.md, "Kiosk mode"): once the app is the device owner
 * (`adb shell dpm set-device-owner …`, possible only on a tablet without
 * accounts), it locks itself in front (lock task), becomes the home app,
 * and turns off the lock screen and the status bar.
 *
 * Leaving it ([exit]) undoes every policy and gives up the device owner
 * role for good: the tablet is an ordinary one again, the app can be
 * uninstalled, and only `adb` can set it again.
 */
object Kiosk {
    private val log = Logger.getLogger("Kiosk")

    private val _active = MutableStateFlow(false)

    /** The app is the device owner, so kiosk mode is on. */
    val active: StateFlow<Boolean> = _active.asStateFlow()

    /**
     * Ends the lock task of the activity in front, set by it while it is
     * resumed. Called before the allow-list is cleared: Android 7 closes a
     * locked task that is no longer allowed, and the system launcher came
     * to the front (#24); a task no longer locked is left alone.
     */
    @Volatile
    var stopLockTask: (() -> Unit)? = null

    private fun dpm(context: Context) = context.getSystemService(DevicePolicyManager::class.java)

    private fun admin(context: Context) = ComponentName(context, KioskAdmin::class.java)

    private fun home(context: Context) = ComponentName(context, "${context.packageName}.HomeAlias")

    /** Starts the app as the home entry, where kiosk mode runs (see MainActivity). */
    fun homeIntent(context: Context): Intent = Intent(Intent.ACTION_MAIN)
        .addCategory(Intent.CATEGORY_HOME)
        .setComponent(home(context))
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /**
     * Live screens started as the home entry (main thread), in front or not:
     * the screen is off, or a system window covers it (#32). Android closes
     * them when the entry is disabled, so it waits until they are gone.
     */
    var homeScreens = 0

    /** Reads the device owner state and applies the kiosk policies when it is set. */
    fun refresh(context: Context) {
        val owner = dpm(context).isDeviceOwnerApp(context.packageName)
        if (owner) {
            apply(context)
        } else if (homeScreens == 0) {
            // Left while a home entry's screen was alive, and the app stopped
            // before that screen handed over.
            setHome(context, enabled = false)
        }
        _active.value = owner
    }

    /** Kiosk mode left: the home entry's screen has handed over, the entry goes. */
    fun disableHome(context: Context) {
        if (!_active.value) setHome(context, enabled = false)
    }

    private fun apply(context: Context) {
        val dpm = dpm(context)
        val admin = admin(context)
        dpm.setLockTaskPackages(admin, arrayOf(context.packageName))
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            // Nothing but the power menu (to restart or turn off the tablet).
            dpm.setLockTaskFeatures(admin, DevicePolicyManager.LOCK_TASK_FEATURE_GLOBAL_ACTIONS)
        }
        dpm.setKeyguardDisabled(admin, true)
        dpm.setStatusBarDisabled(admin, true)
        if (LocalNetwork.required) {
            // Android 17: Wi-Fi to the PC without asking anyone in front of
            // the kiosk (a device owner may grant runtime permissions).
            val granted =
                runCatching {
                    dpm.setPermissionGrantState(
                        admin,
                        context.packageName,
                        LocalNetwork.PERMISSION,
                        DevicePolicyManager.PERMISSION_GRANT_STATE_GRANTED
                    )
                }.getOrDefault(false)
            if (!granted) log.warning("could not grant the local network permission")
            LocalNetwork.refresh(context)
        }
        setHome(context, enabled = true)
        val filter =
            IntentFilter(Intent.ACTION_MAIN).apply {
                addCategory(Intent.CATEGORY_HOME)
                addCategory(Intent.CATEGORY_DEFAULT)
            }
        dpm.addPersistentPreferredActivity(admin, filter, home(context))
    }

    /**
     * Leaves kiosk mode for good. The activity in front first ends its lock
     * task ([stopLockTask]), then the policies and the allow-list are
     * cleared: the app stays in front as a normal app. [by] is logged here
     * and kept until the PC is told ([KioskExits]).
     */
    fun exit(context: Context, by: KioskExitOrigin) {
        val dpm = dpm(context)
        if (!dpm.isDeviceOwnerApp(context.packageName)) {
            _active.value = false
            return
        }
        log.info("leaving kiosk mode, asked by: ${describe(by)}")
        KioskExits(context).record(by)
        val admin = admin(context)
        stopLockTask?.invoke()
        runCatching {
            dpm.clearPackagePersistentPreferredActivities(admin, context.packageName)
            dpm.setKeyguardDisabled(admin, false)
            dpm.setStatusBarDisabled(admin, false)
            dpm.setLockTaskPackages(admin, emptyArray())
        }.onFailure { log.warning("undoing kiosk policies: $it") }
        // Android closes the screens of a disabled home entry: such a screen
        // first reopens the app as itself (at once in front, otherwise when
        // it comes back), then disables it (MainActivity.followKiosk,
        // disableHome).
        if (homeScreens == 0) setHome(context, enabled = false)
        @Suppress("DEPRECATION") // the only way for a device owner to give up the role
        dpm.clearDeviceOwnerApp(context.packageName)
        log.info("kiosk mode left (${describe(by)}): no longer the device owner")
        _active.value = false
    }

    private fun describe(by: KioskExitOrigin) = when (by) {
        KioskExitOrigin.Gesture -> "hidden gesture on the tablet"
        KioskExitOrigin.Pc -> "the PC (Libérer la tablette)"
    }

    /** The home entry exists only in kiosk mode, so a normal tablet never asks which home app to use. */
    private fun setHome(context: Context, enabled: Boolean) {
        context.packageManager.setComponentEnabledSetting(
            home(context),
            if (enabled) {
                PackageManager.COMPONENT_ENABLED_STATE_ENABLED
            } else {
                PackageManager.COMPONENT_ENABLED_STATE_DISABLED
            },
            PackageManager.DONT_KILL_APP
        )
    }
}

/**
 * The last exit from kiosk mode not reported to the PC yet, kept on disk so
 * an exit made away from the PC (or before the app restarted) still reaches
 * the PC's log at the next connection.
 */
class KioskExits(context: Context) : KioskExitLog {
    private val prefs = context.applicationContext.getSharedPreferences("kiosk_exit", Context.MODE_PRIVATE)

    fun record(by: KioskExitOrigin) {
        // Written before the device owner role is given up (the app may be closed right after).
        prefs.edit(commit = true) {
            putString(BY, by.string)
            putLong(AT, System.currentTimeMillis())
        }
    }

    override fun pending(): KioskLeft? {
        val by = KioskExitOrigin.entries.firstOrNull { it.string == prefs.getString(BY, null) } ?: return null
        val seconds = (System.currentTimeMillis() - prefs.getLong(AT, 0L)) / 1000
        return KioskLeft(by, seconds.coerceIn(0L, UInt.MAX_VALUE.toLong()).toUInt())
    }

    override fun reported() {
        prefs.edit { clear() }
    }

    private companion object {
        const val BY = "by"
        const val AT = "at"
    }
}

/**
 * An update stops the app, and the tablet then shows the regular launcher
 * until the home button is pressed: in kiosk mode, the app comes back at
 * once as the home entry (a device owner may start an activity from the
 * background).
 */
class KioskUpdated : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        val dpm = context.getSystemService(DevicePolicyManager::class.java)
        if (!dpm.isDeviceOwnerApp(context.packageName)) return
        context.startActivity(Kiosk.homeIntent(context))
    }
}

/** Device admin component, required to be the device owner. */
class KioskAdmin : DeviceAdminReceiver() {
    override fun onEnabled(context: Context, intent: Intent) = Kiosk.refresh(context)
}
