package io.github.hugoambiehl.padboard.ui.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.ConnectionState
import io.github.hugoambiehl.padboard.connection.FailureReason
import io.github.hugoambiehl.padboard.connection.ServerAddress
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType

private enum class FieldError { Host, Port }

/** Manual connection by IP address (phase 1). Layout follows Pairing.dc.html. */
@Composable
fun ConnectScreen(
    state: ConnectionState,
    savedAddress: ServerAddress?,
    onConnect: (ServerAddress) -> Unit,
    onCancel: () -> Unit
) {
    Row(
        modifier =
        Modifier
            .fillMaxSize()
            .background(PadboardColors.Background)
            .padding(horizontal = 64.dp, vertical = 56.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp)
    ) {
        Column(
            modifier = Modifier.weight(1f).fillMaxHeight(),
            verticalArrangement = Arrangement.spacedBy(28.dp, Alignment.CenterVertically)
        ) {
            Box(
                Modifier.size(52.dp).clip(RoundedCornerShape(16.dp)).background(PadboardColors.Accent),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    painterResource(R.drawable.ic_layout_grid),
                    null,
                    Modifier.size(26.dp),
                    tint = PadboardColors.Background
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.connect_title), style = PadboardType.Display, color = PadboardColors.Text)
                Text(
                    stringResource(R.string.connect_body),
                    style = PadboardType.Body,
                    color = PadboardColors.TextMuted2,
                    modifier = Modifier.widthIn(max = 440.dp)
                )
            }
        }
        AddressCard(
            state = state,
            savedAddress = savedAddress,
            onConnect = onConnect,
            onCancel = onCancel,
            modifier = Modifier.width(500.dp).fillMaxHeight()
        )
    }
}

@Composable
private fun AddressCard(
    state: ConnectionState,
    savedAddress: ServerAddress?,
    onConnect: (ServerAddress) -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier
) {
    var host by rememberSaveable(savedAddress) { mutableStateOf(savedAddress?.host.orEmpty()) }
    var port by rememberSaveable(savedAddress) {
        mutableStateOf((savedAddress?.port ?: ServerAddress.DEFAULT_PORT).toString())
    }
    var fieldError by rememberSaveable { mutableStateOf<FieldError?>(null) }
    val connecting = state is ConnectionState.Connecting

    fun submit(hostValue: String = host) {
        when (val result = ServerAddress.parse(hostValue, port)) {
            is ServerAddress.ParseResult.Valid -> {
                fieldError = null
                onConnect(result.address)
            }
            ServerAddress.ParseResult.InvalidHost -> fieldError = FieldError.Host
            ServerAddress.ParseResult.InvalidPort -> fieldError = FieldError.Port
        }
    }

    val shape = RoundedCornerShape(28.dp)
    Column(
        modifier =
        modifier
            .clip(shape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
            .verticalScroll(rememberScrollState())
            .padding(40.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Icon(painterResource(R.drawable.ic_monitor), null, Modifier.size(24.dp), tint = PadboardColors.Accent)
            Text(
                stringResource(R.string.connect_card_title),
                style = PadboardType.CardTitle,
                color = PadboardColors.Text
            )
        }
        OutlinedTextField(
            value = host,
            onValueChange = { host = it.trim() },
            label = { Text(stringResource(R.string.connect_host_label)) },
            singleLine = true,
            enabled = !connecting,
            isError = fieldError == FieldError.Host,
            supportingText = fieldError.takeIf {
                it == FieldError.Host
            }?.let { { Text(stringResource(R.string.connect_invalid_host)) } },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next),
            colors = fieldColors(),
            shape = RoundedCornerShape(14.dp),
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = port,
            onValueChange = { value -> port = value.filter(Char::isDigit).take(5) },
            label = { Text(stringResource(R.string.connect_port_label)) },
            singleLine = true,
            enabled = !connecting,
            isError = fieldError == FieldError.Port,
            supportingText = fieldError.takeIf {
                it == FieldError.Port
            }?.let { { Text(stringResource(R.string.connect_invalid_port)) } },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { submit() }),
            colors = fieldColors(),
            shape = RoundedCornerShape(14.dp),
            modifier = Modifier.fillMaxWidth()
        )

        StatusMessage(state)

        if (connecting) {
            OutlinedButton(
                onClick = onCancel,
                modifier = Modifier.fillMaxWidth().height(56.dp),
                shape = RoundedCornerShape(14.dp)
            ) {
                Text(stringResource(R.string.connect_cancel), color = PadboardColors.Text)
            }
        } else {
            Button(
                onClick = { submit() },
                modifier = Modifier.fillMaxWidth().height(56.dp),
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = PadboardColors.Accent,
                    contentColor = PadboardColors.Background
                )
            ) {
                Text(stringResource(R.string.connect_button), style = PadboardType.TileLabel)
            }
            OutlinedButton(
                onClick = {
                    host = ServerAddress.LOOPBACK
                    submit(ServerAddress.LOOPBACK)
                },
                modifier = Modifier.fillMaxWidth().height(56.dp),
                shape = RoundedCornerShape(14.dp)
            ) {
                Text(stringResource(R.string.connect_usb_button), color = PadboardColors.Text)
            }
        }
    }
}

@Composable
private fun StatusMessage(state: ConnectionState) {
    when (state) {
        is ConnectionState.Connecting ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                CircularProgressIndicator(Modifier.size(16.dp), color = PadboardColors.TextMuted, strokeWidth = 2.dp)
                Text(
                    stringResource(R.string.connect_connecting, state.address.display),
                    style = PadboardType.Small,
                    color = PadboardColors.TextMuted
                )
            }
        is ConnectionState.Failed ->
            Text(
                text =
                when (state.reason) {
                    FailureReason.Unreachable -> stringResource(R.string.failure_unreachable, state.address.display)
                    FailureReason.Incompatible -> stringResource(R.string.failure_incompatible)
                    FailureReason.Rejected -> stringResource(R.string.failure_rejected)
                    FailureReason.Lost -> stringResource(R.string.failure_lost)
                },
                style = PadboardType.Small,
                color = PadboardColors.Red
            )
        ConnectionState.Idle, is ConnectionState.Connected -> Unit
    }
}

@Composable
private fun fieldColors() = OutlinedTextFieldDefaults.colors(
    focusedBorderColor = PadboardColors.Accent,
    unfocusedBorderColor = PadboardColors.Border2,
    focusedLabelColor = PadboardColors.Accent,
    unfocusedLabelColor = PadboardColors.TextMuted,
    focusedContainerColor = PadboardColors.Background,
    unfocusedContainerColor = PadboardColors.Background,
    disabledContainerColor = PadboardColors.Background,
    cursorColor = PadboardColors.Accent
)
