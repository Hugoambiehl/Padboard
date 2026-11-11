package io.github.hugoambiehl.padboard.kiosk

import android.app.admin.DeviceAdminReceiver
import android.app.admin.DevicePolicyManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import java.util.logging.Logger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Kiosk mode (see README, "Kiosk mode"): once the app is the device owner
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

    private fun dpm(context: Context) = context.getSystemService(DevicePolicyManager::class.java)

    private fun admin(context: Context) = ComponentName(context, KioskAdmin::class.java)

    private fun home(context: Context) = ComponentName(context, "${context.packageName}.HomeAlias")

    /** Reads the device owner state and applies the kiosk policies when it is set. */
    fun refresh(context: Context) {
        val owner = dpm(context).isDeviceOwnerApp(context.packageName)
        if (owner) apply(context)
        _active.value = owner
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
        setHome(context, enabled = true)
        val filter =
            IntentFilter(Intent.ACTION_MAIN).apply {
                addCategory(Intent.CATEGORY_HOME)
                addCategory(Intent.CATEGORY_DEFAULT)
            }
        dpm.addPersistentPreferredActivity(admin, filter, home(context))
    }

    /**
     * Leaves kiosk mode for good. The lock task ends with the allow-list
     * cleared; the caller's activity then shows as a normal app.
     */
    fun exit(context: Context) {
        val dpm = dpm(context)
        if (!dpm.isDeviceOwnerApp(context.packageName)) {
            _active.value = false
            return
        }
        val admin = admin(context)
        runCatching {
            dpm.clearPackagePersistentPreferredActivities(admin, context.packageName)
            dpm.setKeyguardDisabled(admin, false)
            dpm.setStatusBarDisabled(admin, false)
            dpm.setLockTaskPackages(admin, emptyArray())
        }.onFailure { log.warning("undoing kiosk policies: $it") }
        setHome(context, enabled = false)
        @Suppress("DEPRECATION") // the only way for a device owner to give up the role
        dpm.clearDeviceOwnerApp(context.packageName)
        log.info("kiosk mode left: no longer the device owner")
        _active.value = false
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

/** Device admin component, required to be the device owner. */
class KioskAdmin : DeviceAdminReceiver() {
    override fun onEnabled(context: Context, intent: Intent) = Kiosk.refresh(context)
}
