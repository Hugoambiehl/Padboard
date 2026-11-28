package io.github.hugoambiehl.padboard.connection

import android.app.Application
import android.os.Build
import android.provider.Settings
import io.github.hugoambiehl.padboard.BuildConfig
import io.github.hugoambiehl.padboard.kiosk.Kiosk
import io.github.hugoambiehl.padboard.kiosk.KioskExits
import io.github.hugoambiehl.padboard.protocol.KioskExitOrigin
import io.github.hugoambiehl.padboard.protocol.TabletOrder
import io.github.hugoambiehl.padboard.screen.ScreenDecoders
import io.github.hugoambiehl.padboard.update.AndroidUpdater
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

/**
 * The tablet's one connection to the PC, shared by every screen of the
 * process ([acquire] / [release]): it lives while one of them is there.
 *
 * Android can run two instances of the activity in one process: in kiosk
 * mode on Android 12L, the one started by the PC (a standard task) and the
 * home entry (`HomeAlias`, a home task) started by the system when the
 * screen wakes up. With a connection each, both sessions of the same tablet
 * took the PC from each other twice a second. One per process cannot.
 */
class TabletLink private constructor(application: Application) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val okHttp =
        OkHttpClient
            .Builder()
            .connectTimeout(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .build()
    val directory = NsdDirectory(application)
    val manager =
        ConnectionManager(
            parentScope = scope,
            http = okHttp,
            pairing = PairingStore(application),
            directory = directory,
            deviceName = deviceName(application),
            updater = AndroidUpdater(application),
            kioskExits = KioskExits(application),
            config = ConnectionManager.Config(usbEndpoint = Endpoint(Endpoint.LOOPBACK, BuildConfig.USB_PORT)),
            localNetwork = { LocalNetwork.allowed.value },
            screenCapability = { ScreenDecoders.capability(application) }
        )

    init {
        LocalNetwork.refresh(application)
        // mDNS only with the local network allowed: on Android 17 searching
        // without it opens the system's device picker over the app (#31).
        scope.launch {
            LocalNetwork.allowed.collect { allowed ->
                if (allowed) directory.start() else directory.stop()
                manager.nudge()
            }
        }
        scope.launch { manager.start(DeviceIdStore(application).deviceId()) }
        // A PC appearing on the network is worth a reconnection attempt right away.
        scope.launch { directory.pcs.collect { manager.nudge() } }
        scope.launch { Kiosk.active.collect(manager::setKiosk) }
        scope.launch {
            manager.orders.collect { order ->
                when (order) {
                    TabletOrder.ExitKiosk -> Kiosk.exit(application, KioskExitOrigin.Pc)
                }
            }
        }
    }

    private fun close() {
        scope.cancel()
        directory.stop()
        okHttp.dispatcher.executorService.shutdown()
        okHttp.connectionPool.evictAll()
    }

    companion object {
        private const val CONNECT_TIMEOUT_SECONDS = 4L

        private var shared: TabletLink? = null
        private var users = 0

        /** The process's connection, started by its first user. */
        @Synchronized
        fun acquire(application: Application): TabletLink {
            users++
            return shared ?: TabletLink(application).also { shared = it }
        }

        /** Its last user gone, the connection is closed. */
        @Synchronized
        fun release() {
            users--
            if (users == 0) {
                shared?.close()
                shared = null
            }
        }

        /** "Galaxy Tab A" (user-visible device name) when available, else the model ("SM-T580"). */
        private fun deviceName(application: Application): String = Settings.Global
            .getString(application.contentResolver, Settings.Global.DEVICE_NAME)
            ?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
    }
}
