package io.github.hugoambiehl.padboard.ui

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.hugoambiehl.padboard.connection.DiscoveredPc
import io.github.hugoambiehl.padboard.connection.Endpoint
import io.github.hugoambiehl.padboard.connection.LinkState
import io.github.hugoambiehl.padboard.connection.LocalNetwork
import io.github.hugoambiehl.padboard.connection.TabletLink
import io.github.hugoambiehl.padboard.kiosk.Kiosk
import io.github.hugoambiehl.padboard.protocol.KioskExitOrigin
import io.github.hugoambiehl.padboard.ui.grid.PageMemory
import kotlinx.coroutines.launch

/** One screen's view of the process's connection ([TabletLink], shared by every screen). */
class PadboardViewModel(application: Application) : AndroidViewModel(application) {
    private val shared = TabletLink.acquire(application)
    private val manager = shared.manager

    val link = manager.state
    val layout = manager.layout
    val actionFailures = manager.actionFailures
    val navigations = manager.navigations
    val pairingProblems = manager.pairingProblems
    val settings = manager.settings
    val systemStatus = manager.systemStatus
    val kiosk = Kiosk.active

    /** Android 17: Wi-Fi and mDNS allowed (see LocalNetwork). */
    val localNetwork = LocalNetwork.allowed

    /** An update sent by the PC (kiosk mode). */
    val update = manager.update

    /** The page the user is on, kept across reconnections (not app restarts). */
    val pageMemory = PageMemory()

    /** PCs announcing Padboard on the network. */
    val pcs = shared.directory.pcs

    init {
        // Main dispatcher, like the grid that reads the memory.
        viewModelScope.launch {
            manager.state.collect { if (it !is LinkState.Connected) pageMemory.onConnectionLost() }
        }
    }

    fun press(buttonId: String) = manager.press(buttonId)

    fun release(buttonId: String) = manager.release(buttonId)

    fun pair(pc: DiscoveredPc) = manager.pair(pc)

    fun pair(endpoint: Endpoint) = manager.pair(endpoint)

    fun submitPin(pin: String) = manager.submitPin(pin)

    fun cancelPairing() = manager.cancelPairing()

    fun forget() = manager.forget()

    fun retry() = manager.retry()

    /** The hidden gesture, confirmed: kiosk mode is left for good. */
    fun exitKioskByGesture() = Kiosk.exit(getApplication(), KioskExitOrigin.Gesture)

    /** "Réessayer maintenant" while the PC is out of reach. */
    fun retryNow() = manager.nudge()

    /** Hides a failed update. */
    fun dismissUpdate() = manager.dismissUpdate()

    override fun onCleared() {
        TabletLink.release()
    }
}
