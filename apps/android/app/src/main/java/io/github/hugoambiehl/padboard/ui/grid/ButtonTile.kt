package io.github.hugoambiehl.padboard.ui.grid

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import io.github.hugoambiehl.padboard.ui.theme.colors

private val TileShape = RoundedCornerShape(22.dp)
private val IconShape = RoundedCornerShape(16.dp)
private const val PRESS_MS = 120
private const val PRESSED_SCALE = 0.94f
private const val FLASH_ALPHA = 0.22f

/**
 * One button of the grid. A touch sends `button:press` immediately and
 * `button:release` when the finger lifts (needed for long presses later).
 *
 * Press feedback follows the tablet animation rule: only transform and alpha,
 * through `graphicsLayer` (no layout animation, no blur).
 */
@Composable
fun ButtonTile(button: ButtonView, onPress: () -> Unit, onRelease: () -> Unit, modifier: Modifier = Modifier) {
    val colors = button.category.colors()
    var pressed by remember { mutableStateOf(false) }
    val currentOnPress by rememberUpdatedState(onPress)
    val currentOnRelease by rememberUpdatedState(onRelease)

    val scale by animateFloatAsState(if (pressed) PRESSED_SCALE else 1f, tween(PRESS_MS), label = "scale")
    val flash by animateFloatAsState(
        targetValue = if (pressed) FLASH_ALPHA else 0f,
        animationSpec = tween(if (pressed) PRESS_MS / 2 else PRESS_MS * 2),
        label = "flash"
    )

    Box(
        modifier =
        modifier
            // State is read inside the lambdas: only the draw phase reruns.
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }.clip(TileShape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, TileShape)
            .semantics {
                role = Role.Button
                onClick {
                    currentOnPress()
                    currentOnRelease()
                    true
                }
            }.pointerInput(button.id) {
                detectTapGestures(
                    onPress = {
                        pressed = true
                        currentOnPress()
                        try {
                            tryAwaitRelease()
                        } finally {
                            // Also runs if the gesture is cancelled: never leave a key held on the PC.
                            pressed = false
                            currentOnRelease()
                        }
                    }
                )
            }
    ) {
        // Color flash overlay, animated through its alpha only.
        Box(
            Modifier
                .matchParentSize()
                .graphicsLayer { alpha = flash }
                .background(colors.foreground)
        )
        Column(
            modifier = Modifier.fillMaxSize().padding(20.dp),
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            // Icons arrive with a later protocol version; the label's initial stands in for now.
            Box(
                modifier = Modifier.size(56.dp).clip(IconShape).background(colors.background),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = button.label.firstOrNull()?.uppercase() ?: "",
                    style = PadboardType.Initial,
                    color = colors.foreground
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    text = button.label,
                    style = PadboardType.TileLabel,
                    color = PadboardColors.Text,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
                button.subtitle?.let {
                    Text(
                        text = it,
                        style = PadboardType.TileSubtitle,
                        color = PadboardColors.TextMuted,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }
        }
    }
}
