package io.github.hugoambiehl.padboard.ui.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.DiscoveredPc
import io.github.hugoambiehl.padboard.connection.Endpoint
import io.github.hugoambiehl.padboard.connection.PairingProblem
import io.github.hugoambiehl.padboard.protocol.PROTOCOL_VERSION
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import kotlinx.coroutines.flow.Flow

/** First screen when no PC is paired (layout of Pairing.dc.html): PCs found on the network. */
@Composable
fun SetupScreen(
    pcs: List<DiscoveredPc>,
    problems: Flow<PairingProblem>,
    onPick: (DiscoveredPc) -> Unit,
    onManual: (Endpoint) -> Unit,
    onBack: (() -> Unit)?
) {
    var problem by rememberSaveable { mutableStateOf<PairingProblem?>(null) }
    LaunchedEffect(problems) { problems.collect { problem = it } }

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
            AppBadge()
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.setup_title), style = PadboardType.Display, color = PadboardColors.Text)
                Text(
                    stringResource(R.string.setup_body),
                    style = PadboardType.Body,
                    color = PadboardColors.TextMuted2,
                    modifier = Modifier.widthIn(max = 440.dp)
                )
                Text(
                    stringResource(R.string.setup_usb_hint),
                    style = PadboardType.Small,
                    color = PadboardColors.TextMuted,
                    modifier = Modifier.widthIn(max = 440.dp)
                )
            }
            if (onBack != null) {
                TextButton(onClick = onBack) {
                    Text(stringResource(R.string.setup_back), color = PadboardColors.Accent)
                }
            }
        }
        Column(
            modifier =
            Modifier
                .width(500.dp)
                .fillMaxHeight()
                .clip(RoundedCornerShape(28.dp))
                .background(PadboardColors.Surface2)
                .border(1.dp, PadboardColors.Border2, RoundedCornerShape(28.dp))
                .verticalScroll(rememberScrollState())
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(stringResource(R.string.setup_found), style = PadboardType.CardTitle, color = PadboardColors.Text)
            pcs.forEach { pc ->
                PcRow(pc) {
                    problem = null
                    onPick(pc)
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                CircularProgressIndicator(Modifier.size(16.dp), color = PadboardColors.TextMuted, strokeWidth = 2.dp)
                Text(
                    stringResource(R.string.setup_searching),
                    style = PadboardType.Small,
                    color = PadboardColors.TextMuted
                )
            }
            problem?.let {
                Text(
                    stringResource(
                        when (it) {
                            PairingProblem.Unreachable -> R.string.pairing_unreachable
                            PairingProblem.Lost -> R.string.pairing_lost
                        }
                    ),
                    style = PadboardType.Small,
                    color = PadboardColors.Red
                )
            }
            ManualEntry(
                onSubmit = {
                    problem = null
                    onManual(it)
                }
            )
        }
    }
}

@Composable
internal fun AppBadge() {
    Box(
        Modifier.size(52.dp).clip(RoundedCornerShape(16.dp)).background(PadboardColors.Accent),
        contentAlignment = Alignment.Center
    ) {
        Icon(painterResource(R.drawable.ic_layout_grid), null, Modifier.size(26.dp), tint = PadboardColors.Background)
    }
}

@Composable
private fun PcRow(pc: DiscoveredPc, onClick: () -> Unit) {
    val compatible = pc.protocolVersion == null || pc.protocolVersion == PROTOCOL_VERSION.toInt()
    val shape = RoundedCornerShape(18.dp)
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(PadboardColors.Background)
            .border(1.dp, PadboardColors.Border2, shape)
            .clickable(enabled = compatible, onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Icon(painterResource(R.drawable.ic_monitor), null, Modifier.size(24.dp), tint = PadboardColors.Accent)
        Column(Modifier.weight(1f)) {
            Text(
                pc.name,
                style = PadboardType.TileLabel,
                color = PadboardColors.Text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                if (compatible) {
                    pc.hosts.firstOrNull().orEmpty()
                } else {
                    stringResource(R.string.setup_incompatible)
                },
                style = PadboardType.Small,
                color = if (compatible) PadboardColors.TextMuted else PadboardColors.Orange
            )
        }
    }
}

@Composable
private fun ManualEntry(onSubmit: (Endpoint) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    var host by rememberSaveable { mutableStateOf("") }
    var invalid by rememberSaveable { mutableStateOf(false) }
    if (!open) {
        TextButton(onClick = { open = true }) {
            Text(stringResource(R.string.setup_manual), color = PadboardColors.Accent)
        }
        return
    }
    fun submit() {
        when (val result = Endpoint.parse(host, Endpoint.WIFI_PORT.toString())) {
            is Endpoint.ParseResult.Valid -> {
                invalid = false
                onSubmit(result.endpoint)
            }
            else -> invalid = true
        }
    }
    OutlinedTextField(
        value = host,
        onValueChange = { host = it.trim() },
        label = { Text(stringResource(R.string.setup_host_label)) },
        singleLine = true,
        isError = invalid,
        supportingText = if (invalid) ({ Text(stringResource(R.string.setup_invalid_host)) }) else null,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
        keyboardActions = KeyboardActions(onGo = { submit() }),
        colors =
        OutlinedTextFieldDefaults.colors(
            focusedBorderColor = PadboardColors.Accent,
            unfocusedBorderColor = PadboardColors.Border2,
            focusedLabelColor = PadboardColors.Accent,
            unfocusedLabelColor = PadboardColors.TextMuted,
            focusedContainerColor = PadboardColors.Background,
            unfocusedContainerColor = PadboardColors.Background,
            cursorColor = PadboardColors.Accent
        ),
        shape = RoundedCornerShape(14.dp),
        modifier = Modifier.fillMaxWidth()
    )
    Button(
        onClick = ::submit,
        modifier = Modifier.fillMaxWidth().height(56.dp),
        shape = RoundedCornerShape(14.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = PadboardColors.Accent,
            contentColor = PadboardColors.Background
        )
    ) {
        Text(stringResource(R.string.setup_manual_connect), style = PadboardType.TileLabel)
    }
}
