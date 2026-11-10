package io.github.hugoambiehl.padboard.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.hugoambiehl.padboard.connection.LinkState
import io.github.hugoambiehl.padboard.ui.connect.IncompatibleScreen
import io.github.hugoambiehl.padboard.ui.connect.PinScreen
import io.github.hugoambiehl.padboard.ui.connect.SearchingScreen
import io.github.hugoambiehl.padboard.ui.connect.SetupScreen
import io.github.hugoambiehl.padboard.ui.grid.GridScreen
import io.github.hugoambiehl.padboard.ui.grid.GridStatus

/** Root: picks the screen from the connection state. */
@Composable
fun PadboardApp(viewModel: PadboardViewModel) {
    val link by viewModel.link.collectAsStateWithLifecycle()
    val layout by viewModel.layout.collectAsStateWithLifecycle()
    val pcs by viewModel.pcs.collectAsStateWithLifecycle()

    /** The user asked to pair over Wi-Fi while connected over USB. */
    var setupRequested by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(link is LinkState.Pairing) { if (link is LinkState.Pairing) setupRequested = false }

    val setup = @Composable { onBack: (() -> Unit)? ->
        SetupScreen(
            pcs = pcs,
            problems = viewModel.pairingProblems,
            onPick = viewModel::pair,
            onManual = viewModel::pair,
            onBack = onBack
        )
    }

    when (val state = link) {
        LinkState.Starting -> SearchingScreen(serverName = null, onForget = null)
        LinkState.Unpaired -> setup(null)
        LinkState.Incompatible -> IncompatibleScreen(onRetry = viewModel::retry)
        is LinkState.Pairing ->
            PinScreen(state = state, onSubmit = viewModel::submitPin, onCancel = viewModel::cancelPairing)
        is LinkState.Connected ->
            if (setupRequested) {
                setup { setupRequested = false }
            } else {
                GridScreen(
                    layout = layout,
                    status = GridStatus.Connected(state.serverName, state.transport, state.latencyMs, state.paired),
                    actionFailures = viewModel.actionFailures,
                    onPress = viewModel::press,
                    onRelease = viewModel::release,
                    onPairWifi = { setupRequested = true },
                    onForget = viewModel::forget
                )
            }
        is LinkState.Searching ->
            if (setupRequested) {
                setup { setupRequested = false }
            } else if (layout != null) {
                GridScreen(
                    layout = layout,
                    status = GridStatus.Reconnecting(state.serverName),
                    actionFailures = viewModel.actionFailures,
                    onPress = viewModel::press,
                    onRelease = viewModel::release,
                    onPairWifi = { setupRequested = true },
                    onForget = viewModel::forget
                )
            } else {
                SearchingScreen(serverName = state.serverName, onForget = viewModel::forget)
            }
    }
}
