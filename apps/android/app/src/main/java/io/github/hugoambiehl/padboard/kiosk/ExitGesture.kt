package io.github.hugoambiehl.padboard.kiosk

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import kotlinx.coroutines.delay

/** How long the corner must be held to offer leaving kiosk mode. */
const val KIOSK_EXIT_HOLD_MS = 5_000L

/**
 * The hidden corner: a square in the bottom-left corner of the screen.
 * Every screen keeps at least this margin on its left and bottom edges
 * (the grid has 40 dp, the other screens 48 to 64 dp), so no button is ever
 * in it. The top-right corner was used until 0.2.0-rc.1: next to the
 * connection pill, it was triggered by mistake during a pairing.
 */
val KIOSK_EXIT_CORNER: Dp = 40.dp

/** The confirm button stays disabled this long, so a stray touch cannot leave kiosk mode. */
const val KIOSK_EXIT_ARM_MS = 2_000L

/** Unanswered, the confirmation closes by itself: cancelling is the default. */
const val KIOSK_EXIT_ASK_MS = 30_000L

/** A touch at ([x], [y]) on a screen of [width] × [height] starts in the hidden corner. */
fun inKioskExitCorner(x: Float, y: Float, width: Float, height: Float, corner: Float): Boolean =
    x in 0f..corner && y in (height - corner)..height && width > 0f

/**
 * Hidden way out of kiosk mode, on every screen (even without a PC): hold
 * the bottom-left corner for 5 s without moving, then confirm. The touches
 * are only watched, never consumed.
 */
@Composable
fun KioskExitCorner(enabled: Boolean, onExit: () -> Unit, content: @Composable () -> Unit) {
    var asking by remember { mutableStateOf(false) }
    Box(
        Modifier.fillMaxSize().pointerInput(enabled) {
            if (!enabled) return@pointerInput
            val corner = KIOSK_EXIT_CORNER.toPx()
            val slop = viewConfiguration.touchSlop
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                val inCorner =
                    inKioskExitCorner(
                        down.position.x,
                        down.position.y,
                        size.width.toFloat(),
                        size.height.toFloat(),
                        corner
                    )
                if (!inCorner) return@awaitEachGesture
                val ended =
                    withTimeoutOrNull(KIOSK_EXIT_HOLD_MS) {
                        while (true) {
                            val change =
                                awaitPointerEvent(PointerEventPass.Initial).changes.firstOrNull { it.id == down.id }
                            if (change == null || !change.pressed) break
                            if ((change.position - down.position).getDistance() > slop) break
                        }
                    }
                if (ended == null) asking = true
            }
        }
    ) {
        content()
    }
    if (asking && enabled) {
        KioskExitDialog(
            onExit = {
                asking = false
                onExit()
            },
            onCancel = { asking = false }
        )
    }
}

/**
 * "Quitter le mode kiosque ?": the consequences spelled out, a red button
 * armed after [KIOSK_EXIT_ARM_MS], "Annuler" focused, and cancelled by a
 * touch outside, Back, or no answer within [KIOSK_EXIT_ASK_MS].
 */
@Composable
private fun KioskExitDialog(onExit: () -> Unit, onCancel: () -> Unit) {
    var armed by remember { mutableStateOf(false) }
    val cancelFocus = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        runCatching { cancelFocus.requestFocus() }
        delay(KIOSK_EXIT_ARM_MS)
        armed = true
        delay(KIOSK_EXIT_ASK_MS - KIOSK_EXIT_ARM_MS)
        onCancel()
    }
    AlertDialog(
        onDismissRequest = onCancel,
        containerColor = PadboardColors.Surface2,
        modifier = Modifier.testTag("kiosk-exit-dialog"),
        title = { Text(stringResource(R.string.kiosk_exit_title), color = PadboardColors.Text) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                for (line in listOf(
                    R.string.kiosk_exit_unlocked,
                    R.string.kiosk_exit_ordinary,
                    R.string.kiosk_exit_again
                )) {
                    Text(stringResource(line), style = PadboardType.Body, color = PadboardColors.TextMuted2)
                }
                Text(
                    stringResource(R.string.kiosk_exit_mistake),
                    style = PadboardType.Body,
                    color = PadboardColors.Text
                )
            }
        },
        confirmButton = {
            Button(
                onClick = onExit,
                enabled = armed,
                modifier = Modifier.testTag("kiosk-exit-confirm"),
                colors = ButtonDefaults.buttonColors(
                    containerColor = PadboardColors.Red,
                    contentColor = PadboardColors.Background,
                    disabledContainerColor = PadboardColors.Red.copy(alpha = 0.3f),
                    disabledContentColor = PadboardColors.Background.copy(alpha = 0.6f)
                )
            ) { Text(stringResource(R.string.kiosk_exit_confirm)) }
        },
        dismissButton = {
            OutlinedButton(
                onClick = onCancel,
                modifier = Modifier.focusRequester(cancelFocus).testTag("kiosk-exit-cancel")
            ) { Text(stringResource(R.string.cancel), color = PadboardColors.Text) }
        }
    )
}
