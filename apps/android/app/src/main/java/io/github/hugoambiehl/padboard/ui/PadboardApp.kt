package io.github.hugoambiehl.padboard.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.hugoambiehl.padboard.connection.DiscoveredPc
import io.github.hugoambiehl.padboard.connection.LinkState
import io.github.hugoambiehl.padboard.kiosk.KioskExitCorner
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.ui.about.AboutScreen
import io.github.hugoambiehl.padboard.ui.connect.IncompatibleScreen
import io.github.hugoambiehl.padboard.ui.connect.PinScreen
import io.github.hugoambiehl.padboard.ui.connect.SearchingScreen
import io.github.hugoambiehl.padboard.ui.connect.SetupScreen
import io.github.hugoambiehl.padboard.ui.grid.GridScreen
import io.github.hugoambiehl.padboard.ui.grid.GridStatus
import io.github.hugoambiehl.padboard.ui.screensaver.ScreensaverHost

/** Root: picks the screen from the connection state. */
@Composable
fun PadboardApp(viewModel: PadboardViewModel) {
    val link by viewModel.link.collectAsStateWithLifecycle()
    val layout by viewModel.layout.collectAsStateWithLifecycle()
    val pcs by viewModel.pcs.collectAsStateWithLifecycle()
    val kiosk by viewModel.kiosk.collectAsStateWithLifecycle()
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val systemStatus by viewModel.systemStatus.collectAsStateWithLifecycle()
    KioskExitCorner(enabled = kiosk, onExit = viewModel::exitKiosk) {
        ScreensaverHost(settings = settings, status = systemStatus) {
            Screens(viewModel, link, layout, pcs)
        }
    }
}

@Composable
private fun Screens(viewModel: PadboardViewModel, link: LinkState, layout: Layout?, pcs: List<DiscoveredPc>) {
    /** The user asked to pair over Wi-Fi while connected over USB. */
    var setupRequested by rememberSaveable { mutableStateOf(false) }
    var aboutOpen by rememberSaveable { mutableStateOf(false) }
    if (aboutOpen) {
        AboutScreen(onBack = { aboutOpen = false })
        return
    }
    LaunchedEffect(link is LinkState.Pairing) { if (link is LinkState.Pairing) setupRequested = false }

    val setup = @Composable { onBack: (() -> Unit)? ->
        SetupScreen(
            pcs = pcs,
            problems = viewModel.pairingProblems,
            onPick = viewModel::pair,
            onManual = viewModel::pair,
            onBack = onBack,
            onAbout = { aboutOpen = true }
        )
    }

    when (val state = link) {
        LinkState.Starting -> SearchingScreen(serverName = null, onForget = null)
        LinkState.Unpaired -> setup(null)
        LinkState.Incompatible -> IncompatibleScreen(onRetry = viewModel::retry)
        is LinkState.Pairing ->
            PinScreen(state = state, onSubmit = viewModel::submitPin, onCancel = viewModel::cancelPairing)
        is LinkState.Connected, is LinkState.Searching -> {
            // One call site for both, so the grid (and its pager) survives a disconnection.
            val status =
                when (state) {
                    is LinkState.Connected ->
                        GridStatus.Connected(state.serverName, state.transport, state.latencyMs, state.paired)
                    else ->
                        (state as LinkState.Searching).let { s ->
                            layout?.let { GridStatus.Reconnecting(s.serverName, s.wifi, s.retryInMs, s.attempt) }
                        }
                }
            when {
                setupRequested -> setup { setupRequested = false }
                status == null ->
                    SearchingScreen(
                        serverName = (state as LinkState.Searching).serverName,
                        onForget = viewModel::forget.takeIf { (state as LinkState.Searching).wifi },
                        attempt = (state as LinkState.Searching).attempt
                    )
                else ->
                    GridScreen(
                        layout = layout,
                        pageMemory = viewModel.pageMemory,
                        status = status,
                        actionFailures = viewModel.actionFailures,
                        navigations = viewModel.navigations,
                        onPress = viewModel::press,
                        onRelease = viewModel::release,
                        onPairWifi = { setupRequested = true },
                        onForget = viewModel::forget,
                        onAbout = { aboutOpen = true },
                        onRetry = viewModel::retryNow
                    )
            }
        }
    }
}
