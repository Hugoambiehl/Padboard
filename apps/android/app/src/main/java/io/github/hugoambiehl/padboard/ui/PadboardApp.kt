package io.github.hugoambiehl.padboard.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.hugoambiehl.padboard.connection.ConnectionState
import io.github.hugoambiehl.padboard.ui.connect.ConnectScreen
import io.github.hugoambiehl.padboard.ui.grid.GridScreen

/** Root: the grid while connected, the connection screen otherwise. */
@Composable
fun PadboardApp(viewModel: PadboardViewModel) {
    val connection by viewModel.connection.collectAsStateWithLifecycle()
    val layout by viewModel.layout.collectAsStateWithLifecycle()
    val savedAddress by viewModel.savedAddress.collectAsStateWithLifecycle()

    when (val state = connection) {
        is ConnectionState.Connected ->
            GridScreen(
                layout = layout,
                serverName = state.serverName,
                address = state.address,
                actionFailures = viewModel.actionFailures,
                onPress = viewModel::press,
                onRelease = viewModel::release,
                onDisconnect = viewModel::disconnect
            )
        else ->
            ConnectScreen(
                state = state,
                savedAddress = savedAddress,
                onConnect = viewModel::connect,
                onCancel = viewModel::disconnect
            )
    }
}
