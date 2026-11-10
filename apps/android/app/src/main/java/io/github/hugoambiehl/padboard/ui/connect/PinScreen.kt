package io.github.hugoambiehl.padboard.ui.connect

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.LinkState
import io.github.hugoambiehl.padboard.protocol.PairFailureReason
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType

private const val PIN_LENGTH = 6

/** PIN entry (Pairing.dc.html), with the key fingerprint to compare with the PC. */
@Composable
fun PinScreen(state: LinkState.Pairing, onSubmit: (String) -> Unit, onCancel: () -> Unit) {
    var pin by rememberSaveable { mutableStateOf("") }
    // A refused PIN is cleared for the next try.
    LaunchedEffect(state.failure, state.attemptsLeft) { if (state.failure != null) pin = "" }
    val canType = !state.submitting && state.failure.let { it == null || it == PairFailureReason.InvalidPin }

    fun type(digit: Char) {
        if (!canType || pin.length >= PIN_LENGTH) return
        pin += digit
        if (pin.length == PIN_LENGTH) onSubmit(pin)
    }

    Row(
        modifier =
        Modifier
            .fillMaxSize()
            .background(PadboardColors.Background)
            .padding(horizontal = 64.dp, vertical = 48.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp)
    ) {
        Column(
            modifier = Modifier.weight(1f).fillMaxHeight(),
            verticalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterVertically)
        ) {
            AppBadge()
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.pin_title), style = PadboardType.Display, color = PadboardColors.Text)
                Text(
                    stringResource(R.string.pin_server, state.serverName),
                    style = PadboardType.Small,
                    color = PadboardColors.TextMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 560.dp)
                )
            }
            Text(
                stringResource(R.string.pin_body),
                style = PadboardType.Body,
                color = PadboardColors.TextMuted2,
                modifier = Modifier.widthIn(max = 440.dp)
            )
            FingerprintCard(state.fingerprint)
            TextButton(onClick = onCancel) {
                Text(stringResource(R.string.pin_cancel), color = PadboardColors.Accent)
            }
        }
        Column(
            modifier = Modifier.width(440.dp).fillMaxHeight(),
            verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(stringResource(R.string.pin_code_label), style = PadboardType.Small, color = PadboardColors.TextMuted)
            Digits(pin)
            Message(state)
            Keypad(
                enabled = canType,
                onDigit = ::type,
                onErase = { if (canType) pin = pin.dropLast(1) }
            )
        }
    }
}

@Composable
private fun FingerprintCard(fingerprint: String) {
    val shape = RoundedCornerShape(18.dp)
    Column(
        modifier =
        Modifier
            .widthIn(max = 460.dp)
            .clip(shape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(stringResource(R.string.pin_check_title), style = PadboardType.TileLabel, color = PadboardColors.Text)
        Text(
            fingerprint,
            style = PadboardType.CardTitle.copy(fontFamily = FontFamily.Monospace, letterSpacing = 2.sp),
            color = PadboardColors.Teal
        )
        Text(stringResource(R.string.pin_check_hint), style = PadboardType.Small, color = PadboardColors.TextMuted)
    }
}

@Composable
private fun Digits(pin: String) {
    val description = stringResource(R.string.pin_digits_description, pin.length, PIN_LENGTH)
    Row(
        modifier = Modifier.semantics { contentDescription = description },
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        repeat(PIN_LENGTH) { i ->
            val shape = RoundedCornerShape(14.dp)
            val current = i == pin.length
            Box(
                Modifier
                    .size(width = 56.dp, height = 68.dp)
                    .clip(shape)
                    .background(PadboardColors.Surface2)
                    .border(
                        if (current) 2.dp else 1.dp,
                        when {
                            current -> PadboardColors.Accent
                            i < pin.length -> PadboardColors.DotInactive
                            else -> PadboardColors.Border2
                        },
                        shape
                    ),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    pin.getOrNull(i)?.toString().orEmpty(),
                    style = PadboardType.Display.copy(fontSize = 32.sp),
                    color = PadboardColors.Text
                )
            }
        }
    }
}

@Composable
private fun Message(state: LinkState.Pairing) {
    val text =
        when {
            state.submitting -> stringResource(R.string.pin_checking)
            state.failure == PairFailureReason.InvalidPin ->
                pluralStringResource(R.plurals.pin_invalid, state.attemptsLeft ?: 0, state.attemptsLeft ?: 0)
            state.failure == PairFailureReason.Expired -> stringResource(R.string.pin_expired)
            state.failure == PairFailureReason.Locked -> stringResource(R.string.pin_locked)
            state.failure == PairFailureReason.NoActivePin -> stringResource(R.string.pin_no_code)
            else -> ""
        }
    val error = state.failure != null && !state.submitting
    Text(
        text,
        style = PadboardType.Small,
        color = if (error) PadboardColors.Red else PadboardColors.TextMuted,
        modifier = Modifier.height(40.dp)
    )
}

@Composable
private fun Keypad(enabled: Boolean, onDigit: (Char) -> Unit, onErase: () -> Unit) {
    val rows = listOf("123", "456", "789")
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        rows.forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { digit -> Key(digit.toString(), enabled) { onDigit(digit) } }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Spacer(Modifier.size(width = 112.dp, height = 64.dp))
            Key("0", enabled) { onDigit('0') }
            Key(stringResource(R.string.pin_erase), enabled, small = true, onClick = onErase)
        }
    }
}

@Composable
private fun Key(label: String, enabled: Boolean, small: Boolean = false, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    // Transform only, through graphicsLayer (tablet animation rule).
    val scale by animateFloatAsState(if (pressed) 0.93f else 1f, tween(100), label = "key")
    val shape = RoundedCornerShape(16.dp)
    Box(
        modifier =
        Modifier
            .size(width = 112.dp, height = 64.dp)
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
                alpha = if (enabled) 1f else 0.4f
            }.clip(shape)
            .background(if (small) PadboardColors.SurfaceActive else PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
            .clickable(interactionSource = interaction, indication = null, enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = if (small) PadboardType.Small else PadboardType.CardTitle, color = PadboardColors.Text)
    }
}
