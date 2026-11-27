package io.github.hugoambiehl.padboard.connection

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Android 17 (API 37) protects the local network: the Wi-Fi connection to
 * the PC and the mDNS search need `ACCESS_LOCAL_NETWORK` (a runtime
 * permission of the "Nearby devices" group). USB (loopback through
 * `adb reverse`) does not. Without it, searching made the system open its
 * device picker in front of the app at every launch (#31), so the app now
 * asks for it only when Wi-Fi is wanted, and grants it itself in kiosk mode.
 */
object LocalNetwork {
    /** The first Android version that asks for it. */
    const val SDK = 37
    const val PERMISSION = "android.permission.ACCESS_LOCAL_NETWORK"

    private val _allowed = MutableStateFlow(Build.VERSION.SDK_INT < SDK)

    /** Wi-Fi and mDNS may be used (always below Android 17). */
    val allowed: StateFlow<Boolean> = _allowed.asStateFlow()

    /** The permission has to be asked for on this tablet. */
    val required get() = Build.VERSION.SDK_INT >= SDK

    /** Reads it again: after a request, back from the settings, kiosk mode turned on. */
    fun refresh(context: Context) {
        _allowed.value = !required ||
            context.checkSelfPermission(PERMISSION) == PackageManager.PERMISSION_GRANTED
    }
}
