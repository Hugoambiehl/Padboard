package io.github.hugoambiehl.padboard.ui.grid

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.protocol.ButtonIcon
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.protocol.Category
import io.github.hugoambiehl.padboard.ui.theme.CategoryColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import io.github.hugoambiehl.padboard.ui.theme.colors

private val TileShape = RoundedCornerShape(22.dp)
private val IconShape = RoundedCornerShape(16.dp)
private const val PRESS_MS = 120
private const val PRESSED_SCALE = 0.92f
private const val FLASH_ALPHA = 0.3f
private const val UNAVAILABLE_ALPHA = 0.4f
private const val STATE_MS = 200

/** A touch held still this long is a press even before the finger lifts. */
private const val HOLD_MS = 120L

/**
 * One button of the grid. The tile reacts at once on touch, but the grid can
 * be swiped to change page, so the press is only sent once the touch is
 * known not to be a swipe: when the finger lifts without moving (a tap), or
 * after it stayed still for [HOLD_MS] (a hold, for long presses later).
 * `button:release` follows when the finger lifts or the gesture is taken over.
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

    // No vibration motor on the tablet: a marked press, and a small bounce back.
    val scale by animateFloatAsState(
        if (pressed) PRESSED_SCALE else 1f,
        if (pressed) tween(PRESS_MS) else spring(dampingRatio = 0.45f, stiffness = Spring.StiffnessMediumLow),
        label = "scale"
    )
    // State changes fade (alpha only): the active surface and border are an overlay.
    val activeAlpha by animateFloatAsState(if (button.active) 1f else 0f, tween(STATE_MS), label = "active")
    val tileAlpha by animateFloatAsState(
        if (button.unavailable) UNAVAILABLE_ALPHA else 1f,
        tween(STATE_MS),
        label = "available"
    )
    val enabled by rememberUpdatedState(!button.unavailable)
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
                alpha = tileAlpha
            }.clip(TileShape)
            .background(PadboardColors.Surface2)
            .semantics {
                role = Role.Button
                if (button.unavailable) disabled()
                onClick {
                    currentOnPress()
                    currentOnRelease()
                    true
                }
            }.pointerInput(button.id) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    // Unavailable (e.g. OBS closed): no feedback, nothing sent;
                    // the pager still gets the swipe.
                    if (!enabled) return@awaitEachGesture
                    pressed = true
                    var sent = false
                    try {
                        when (awaitTapOrHold(down.id, down.position, viewConfiguration.touchSlop)) {
                            Touch.Tap -> {
                                currentOnPress()
                                sent = true
                            }
                            Touch.Hold -> {
                                currentOnPress()
                                sent = true
                                awaitLiftOrTakeover(down.id)
                            }
                            Touch.Swipe -> Unit
                        }
                    } finally {
                        // Also runs if the gesture is cancelled: never leave a key held on the PC.
                        pressed = false
                        if (sent) currentOnRelease()
                    }
                }
            }
    ) {
        // Resting border, as a layer: `Modifier.border` on the tile would be
        // drawn over its children and hide the active border below.
        Box(Modifier.matchParentSize().border(1.dp, PadboardColors.Border2, TileShape))
        // Active state: active surface and a border in the category color.
        Box(
            Modifier
                .matchParentSize()
                .graphicsLayer { alpha = activeAlpha }
                .background(PadboardColors.SurfaceActive)
                .border(1.dp, colors.foreground, TileShape)
        )
        // Color flash overlay, animated through its alpha only.
        Box(
            Modifier
                .matchParentSize()
                .graphicsLayer { alpha = flash }
                .background(colors.foreground)
        )
        // A new look (another icon, color or text for the state) fades over
        // the old one, alpha only.
        Crossfade(targetState = TileLook(button), animationSpec = tween(STATE_MS), label = "look") { look ->
            Column(
                modifier = Modifier.fillMaxSize().padding(20.dp),
                verticalArrangement = Arrangement.SpaceBetween
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.Top
                ) {
                    Box(
                        modifier = Modifier.size(56.dp).clip(IconShape).background(look.colors.background),
                        contentAlignment = Alignment.Center
                    ) {
                        // Cached by paths: an unchanged icon is not rebuilt on layout updates.
                        val icon = look.icon?.let { IconVectors.get(it.paths) }
                        if (icon != null) {
                            Icon(
                                icon,
                                contentDescription = null,
                                modifier = Modifier.size(28.dp),
                                tint = look.colors.foreground
                            )
                        } else {
                            // No icon (or an unreadable one): the label's initial.
                            Text(
                                text = look.label.firstOrNull()?.uppercase() ?: "",
                                style = PadboardType.Initial,
                                color = look.colors.foreground
                            )
                        }
                    }
                    look.badge?.let { Badge(it, look.colors) }
                }
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(
                        text = look.label,
                        style = PadboardType.TileLabel,
                        color = PadboardColors.Text,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis
                    )
                    look.subtitle?.let {
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
}

/** What a tile shows, besides its active and unavailable states. */
private data class TileLook(
    val icon: ButtonIcon?,
    val category: Category,
    val label: String,
    val subtitle: String?,
    val badge: String?
) {
    constructor(button: ButtonView) : this(button.icon, button.category, button.label, button.subtitle, button.badge)

    val colors: CategoryColors get() = category.colors()
}

private enum class Touch { Tap, Hold, Swipe }

/**
 * Follows the touch until it is a tap (lifted without moving), a hold (still
 * for [HOLD_MS]) or a swipe (moved past the touch slop, or taken over by the
 * pager). Nothing is consumed, so the pager still sees the movement.
 */
private suspend fun AwaitPointerEventScope.awaitTapOrHold(id: PointerId, start: Offset, slop: Float): Touch =
    withTimeoutOrNull(HOLD_MS) { awaitTapOrSwipe(id, start, slop) } ?: Touch.Hold

private suspend fun AwaitPointerEventScope.awaitTapOrSwipe(id: PointerId, start: Offset, slop: Float): Touch {
    while (true) {
        val change = awaitPointerEvent().changes.firstOrNull { it.id == id } ?: return Touch.Swipe
        if (!change.pressed) return Touch.Tap
        if (change.isConsumed || (change.position - start).getDistance() > slop) return Touch.Swipe
    }
}

/** Waits until the finger lifts, or the pager takes the gesture over. */
private suspend fun AwaitPointerEventScope.awaitLiftOrTakeover(id: PointerId) {
    while (true) {
        val change = awaitPointerEvent().changes.firstOrNull { it.id == id } ?: return
        if (!change.pressed || change.isConsumed) return
    }
}

/** "REC", "EN DIRECT"…: tinted pill with a pulsing dot (alpha only), as in Main.dc.html. */
@Composable
private fun Badge(text: String, colors: CategoryColors) {
    val transition = rememberInfiniteTransition(label = "badge")
    val pulse by transition.animateFloat(
        initialValue = 1f,
        targetValue = 0.3f,
        animationSpec = infiniteRepeatable(tween(600), RepeatMode.Reverse),
        label = "badgePulse"
    )
    Row(
        modifier =
        Modifier
            .clip(CircleShape)
            .background(colors.background)
            .padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Box(
            Modifier
                .size(6.dp)
                .graphicsLayer { alpha = pulse }
                .clip(CircleShape)
                .background(colors.foreground)
        )
        Text(text, style = PadboardType.Badge, color = colors.foreground, maxLines = 1)
    }
}
