package io.github.hugoambiehl.padboard.ui.grid

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.ui.connect.SearchIndicator
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import kotlinx.coroutines.delay

/** Shown this long after the link drops: a USB ↔ Wi-Fi hand-over or a blip shows nothing. */
const val DISCONNECTED_GRACE_MS = 1_000L

/** Below this, the next attempt is "now": no countdown flickering by. */
private const val COUNTDOWN_FROM_MS = 2_000L

/**
 * Card over the dimmed grid while the PC is out of reach: who is looked for,
 * how, when the next attempt starts, and a way to try at once. Faded in and
 * out (alpha and scale only); it is composed only while shown or fading.
 */
@Composable
fun DisconnectedCard(
    status: GridStatus.Reconnecting,
    visible: Boolean,
    onRetry: () -> Unit,
    onPairWifi: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shown by animateFloatAsState(if (visible) 1f else 0f, tween(200), label = "disconnected")
    // Hidden: not composed at all, or its spinner would keep asking for
    // frames (60 fps of redraws over an idle grid).
    if (!visible && shown == 0f) return
    var remainingMs by remember { mutableLongStateOf(0L) }
    LaunchedEffect(status.attempt) {
        remainingMs = status.retryInMs
        while (remainingMs > 0) {
            delay(minOf(remainingMs, 1_000L))
            remainingMs -= 1_000L
        }
    }
    val shape = RoundedCornerShape(22.dp)
    Column(
        modifier =
        modifier
            .graphicsLayer {
                alpha = shown
                val scale = 0.96f + 0.04f * shown
                scaleX = scale
                scaleY = scale
            }.layout { measurable, constraints ->
                // Faded out: not placed, so neither drawn nor touchable.
                val placeable = measurable.measure(constraints)
                layout(placeable.width, placeable.height) { if (shown > 0f) placeable.place(0, 0) }
            }.widthIn(max = 560.dp)
            .clip(shape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
            .padding(32.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            SearchIndicator(status.attempt)
            Column {
                Text(
                    stringResource(R.string.disconnected_title),
                    style = PadboardType.CardTitle,
                    color = PadboardColors.Text
                )
                Text(
                    status.serverName,
                    style = PadboardType.Small,
                    color = PadboardColors.TextMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
        Text(
            stringResource(
                when {
                    !status.wifi -> R.string.disconnected_body_usb
                    status.announced -> R.string.disconnected_body_unreachable
                    else -> R.string.disconnected_body_wifi
                }
            ),
            style = PadboardType.Body,
            color = PadboardColors.TextMuted2
        )
        Box(Modifier.padding(top = 4.dp)) {
            val seconds = (remainingMs + 999) / 1_000
            Text(
                if (seconds * 1_000 >= COUNTDOWN_FROM_MS) {
                    stringResource(R.string.disconnected_next_attempt, seconds.toInt())
                } else {
                    stringResource(R.string.disconnected_trying)
                },
                style = PadboardType.Small,
                color = PadboardColors.TextMuted
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(
                onClick = onRetry,
                colors = ButtonDefaults.buttonColors(
                    containerColor = PadboardColors.Accent,
                    contentColor = PadboardColors.Background
                )
            ) { Text(stringResource(R.string.disconnected_retry)) }
            if (!status.wifi) {
                TextButton(onClick = onPairWifi) {
                    Text(stringResource(R.string.pair_wifi), color = PadboardColors.Accent)
                }
            }
        }
    }
}
