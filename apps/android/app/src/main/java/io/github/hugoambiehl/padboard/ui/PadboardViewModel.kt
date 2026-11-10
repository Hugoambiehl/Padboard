package io.github.hugoambiehl.padboard.ui

import android.app.Application
import android.os.Build
import android.provider.Settings
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.hugoambiehl.padboard.connection.ConnectionState
import io.github.hugoambiehl.padboard.connection.PadboardClient
import io.github.hugoambiehl.padboard.connection.ServerAddress
import io.github.hugoambiehl.padboard.connection.ServerAddressStore
import io.github.hugoambiehl.padboard.protocol.Layout
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

class PadboardViewModel(application: Application) : AndroidViewModel(application) {
    private val store = ServerAddressStore(application)
    private val okHttp =
        OkHttpClient
            .Builder()
            .connectTimeout(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .build()
    private val client = PadboardClient(okHttp, deviceName(application))

    val connection: StateFlow<ConnectionState> = client.state
    val layout: StateFlow<Layout?> = client.layout
    val actionFailures: SharedFlow<String> = client.actionFailures

    /** Last used address, to prefill the connection form. */
    val savedAddress: StateFlow<ServerAddress?> =
        store.address.stateIn(viewModelScope, SharingStarted.Eagerly, null)

    init {
        // A dedicated tablet should come back to its PC by itself.
        viewModelScope.launch { store.address.first()?.let(client::connect) }
    }

    fun connect(address: ServerAddress) {
        viewModelScope.launch { store.save(address) }
        client.connect(address)
    }

    fun disconnect() = client.disconnect()

    fun press(buttonId: String) = client.press(buttonId)

    fun release(buttonId: String) = client.release(buttonId)

    override fun onCleared() {
        client.disconnect()
        okHttp.dispatcher.executorService.shutdown()
        okHttp.connectionPool.evictAll()
    }

    private companion object {
        const val CONNECT_TIMEOUT_SECONDS = 5L

        /** "Galaxy Tab A" (user-visible device name) when available, else the model ("SM-T580"). */
        fun deviceName(application: Application): String = Settings.Global
            .getString(application.contentResolver, Settings.Global.DEVICE_NAME)
            ?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
    }
}
