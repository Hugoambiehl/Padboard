package io.github.hugoambiehl.padboard.ui.screensaver

import android.os.SystemClock
import android.view.WindowManager
import androidx.activity.compose.LocalActivity
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.protocol.SystemStatusPayload
import io.github.hugoambiehl.padboard.protocol.TabletSettingsPayload
import io.github.hugoambiehl.padboard.ui.theme.Inter
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.SpaceGrotesk
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import kotlin.random.Random
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first

/** Before the PC sent its settings (never connected since the app started). */
val DEFAULT_SCREENSAVER = TabletSettingsPayload(screensaver = true, screensaverDelaySeconds = 300u)

/** Window brightness while asleep (0 would turn the backlight off: the clock must stay readable). */
private const val ASLEEP_BRIGHTNESS = 0.02f

/** The clock moves this often, so no pixel stays lit for hours. */
private const val MOVE_EVERY_MS = 60_000L

/**
 * Screensaver over every screen: after the PC's delay without a touch, a
 * black screen with the time, the date and the PC's important states
 * (microphone muted, REC, EN DIRECT), the window at its lowest brightness.
 * The touch that wakes it only wakes it: it never reaches the grid.
 *
 * Touches are watched on the way down (never consumed) to know when the
 * tablet was last used. Only alpha and transforms are animated.
 */
@Composable
fun ScreensaverHost(settings: TabletSettingsPayload?, status: SystemStatusPayload?, content: @Composable () -> Unit) {
    val effective = settings ?: DEFAULT_SCREENSAVER
    val enabled = effective.screensaver
    val delayMs = effective.screensaverDelaySeconds.toLong() * 1_000L
    val lastTouch = remember { longArrayOf(SystemClock.uptimeMillis()) }
    var asleep by remember { mutableStateOf(false) }

    LaunchedEffect(enabled, delayMs) {
        asleep = false
        if (!enabled) return@LaunchedEffect
        lastTouch[0] = SystemClock.uptimeMillis()
        while (true) {
            val idle = SystemClock.uptimeMillis() - lastTouch[0]
            if (idle < delayMs) {
                delay(delayMs - idle)
                continue
            }
            asleep = true
            snapshotFlow { asleep }.first { !it }
        }
    }

    Box(
        Modifier.fillMaxSize().pointerInput(Unit) {
            awaitPointerEventScope {
                while (true) {
                    awaitPointerEvent(PointerEventPass.Initial)
                    lastTouch[0] = SystemClock.uptimeMillis()
                }
            }
        }
    ) {
        content()
        ScreensaverOverlay(
            asleep = asleep,
            status = status,
            onWake = {
                lastTouch[0] = SystemClock.uptimeMillis()
                asleep = false
            }
        )
    }
    WindowBrightness(dimmed = asleep)
}

@Composable
private fun ScreensaverOverlay(asleep: Boolean, status: SystemStatusPayload?, onWake: () -> Unit) {
    val shown by animateFloatAsState(
        if (asleep) 1f else 0f,
        tween(if (asleep) 800 else 200),
        label = "screensaver"
    )
    val touch =
        if (asleep) {
            // Catches the whole waking gesture: nothing below sees it.
            Modifier.pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown().consume()
                    onWake()
                    do {
                        val event = awaitPointerEvent()
                        event.changes.forEach { it.consume() }
                    } while (event.changes.any { it.pressed })
                }
            }
        } else {
            Modifier
        }
    BoxWithConstraints(
        Modifier
            .layout { measurable, constraints ->
                // Faded out: not placed, so neither drawn nor touchable.
                val placeable = measurable.measure(constraints)
                layout(placeable.width, placeable.height) { if (shown > 0f) placeable.place(0, 0) }
            }.fillMaxSize()
            .graphicsLayer { alpha = shown }
            .background(Color.Black)
            .then(touch)
            .testTag("screensaver"),
        contentAlignment = Alignment.Center
    ) {
        if (shown == 0f) return@BoxWithConstraints
        // Each minute, the clock drifts to another spot (moved, never laid out again).
        val rangeX = constraints.maxWidth * 0.28f
        val rangeY = constraints.maxHeight * 0.25f
        val offsetX = remember { Animatable(0f) }
        val offsetY = remember { Animatable(0f) }
        var now by remember { mutableStateOf(LocalDateTime.now()) }
        LaunchedEffect(Unit) {
            while (true) {
                now = LocalDateTime.now()
                val nextMinute = now.truncatedTo(ChronoUnit.MINUTES).plusMinutes(1)
                delay(ChronoUnit.MILLIS.between(now, nextMinute).coerceIn(1L, MOVE_EVERY_MS))
                now = LocalDateTime.now()
                offsetX.animateTo(Random.nextFloat() * 2 * rangeX - rangeX, tween(2_000))
                offsetY.animateTo(Random.nextFloat() * 2 * rangeY - rangeY, tween(2_000))
            }
        }
        Clock(
            now = now,
            status = status,
            modifier = Modifier.graphicsLayer {
                translationX = offsetX.value
                translationY = offsetY.value
            }
        )
    }
}

private val TIME = DateTimeFormatter.ofPattern("HH:mm")
private val DATE = DateTimeFormatter.ofPattern("EEEE d MMMM")

@Composable
private fun Clock(now: LocalDateTime, status: SystemStatusPayload?, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            TIME.format(now),
            style = TextStyle(fontFamily = SpaceGrotesk, fontWeight = FontWeight.Medium, fontSize = 128.sp),
            color = PadboardColors.TextMuted2
        )
        Text(
            DATE.format(now).replaceFirstChar { it.uppercase() },
            style = TextStyle(fontFamily = Inter, fontSize = 22.sp),
            color = PadboardColors.TextMuted
        )
        Row(
            horizontalArrangement = Arrangement.spacedBy(20.dp),
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.testTag("screensaver-states")
        ) {
            when {
                status == null -> State(stringResource(R.string.screensaver_offline), PadboardColors.TextMuted)
                else -> {
                    if (status.microphoneMuted) {
                        State(
                            stringResource(R.string.screensaver_mic_muted),
                            PadboardColors.Orange
                        )
                    }
                    if (status.recording) State(stringResource(R.string.screensaver_recording), PadboardColors.Red)
                    if (status.streaming) State(stringResource(R.string.screensaver_live), PadboardColors.Red)
                }
            }
        }
    }
}

/** One discreet state: a dot and a word, dimmed. */
@Composable
private fun State(text: String, color: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.size(8.dp).background(color.copy(alpha = 0.8f), CircleShape))
        Text(
            text,
            style = TextStyle(
                fontFamily = Inter,
                fontWeight = FontWeight.SemiBold,
                fontSize = 16.sp,
                letterSpacing = 1.sp
            ),
            color = color.copy(alpha = 0.8f)
        )
    }
}

/** Lowest brightness while asleep, the user's own brightness otherwise. */
@Composable
private fun WindowBrightness(dimmed: Boolean) {
    val window = LocalActivity.current?.window ?: return
    DisposableEffect(window, dimmed) {
        fun set(value: Float) {
            window.attributes = window.attributes.apply { screenBrightness = value }
        }
        set(if (dimmed) ASLEEP_BRIGHTNESS else WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE)
        onDispose { set(WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE) }
    }
}
