package io.github.hugoambiehl.padboard.ui

import android.app.Application
import android.os.Build
import android.provider.Settings
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.hugoambiehl.padboard.connection.ConnectionManager
import io.github.hugoambiehl.padboard.connection.DeviceIdStore
import io.github.hugoambiehl.padboard.connection.DiscoveredPc
import io.github.hugoambiehl.padboard.connection.Endpoint
import io.github.hugoambiehl.padboard.connection.NsdDirectory
import io.github.hugoambiehl.padboard.connection.PairingStore
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

class PadboardViewModel(application: Application) : AndroidViewModel(application) {
    private val okHttp =
        OkHttpClient
            .Builder()
            .connectTimeout(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .build()
    private val directory = NsdDirectory(application)
    private val manager =
        ConnectionManager(
            parentScope = viewModelScope,
            http = okHttp,
            pairing = PairingStore(application),
            directory = directory,
            deviceName = deviceName(application)
        )

    val link = manager.state
    val layout = manager.layout
    val actionFailures = manager.actionFailures
    val pairingProblems = manager.pairingProblems

    /** PCs announcing Padboard on the network. */
    val pcs = directory.pcs

    init {
        directory.start()
        viewModelScope.launch { manager.start(DeviceIdStore(application).deviceId()) }
        // A PC appearing on the network is worth a reconnection attempt right away.
        viewModelScope.launch { directory.pcs.collect { manager.nudge() } }
    }

    fun press(buttonId: String) = manager.press(buttonId)

    fun release(buttonId: String) = manager.release(buttonId)

    fun pair(pc: DiscoveredPc) = manager.pair(pc)

    fun pair(endpoint: Endpoint) = manager.pair(endpoint)

    fun submitPin(pin: String) = manager.submitPin(pin)

    fun cancelPairing() = manager.cancelPairing()

    fun forget() = manager.forget()

    fun retry() = manager.retry()

    override fun onCleared() {
        directory.stop()
        okHttp.dispatcher.executorService.shutdown()
        okHttp.connectionPool.evictAll()
    }

    private companion object {
        const val CONNECT_TIMEOUT_SECONDS = 4L

        /** "Galaxy Tab A" (user-visible device name) when available, else the model ("SM-T580"). */
        fun deviceName(application: Application): String = Settings.Global
            .getString(application.contentResolver, Settings.Global.DEVICE_NAME)
            ?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
    }
}
