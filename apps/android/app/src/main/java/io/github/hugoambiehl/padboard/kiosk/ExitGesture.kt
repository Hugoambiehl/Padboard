package io.github.hugoambiehl.padboard.kiosk

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors

/** How long the top-right corner must be held to offer leaving kiosk mode. */
const val KIOSK_EXIT_HOLD_MS = 5_000L

/**
 * Hidden way out of kiosk mode, on every screen (even without a PC): hold
 * the top-right corner (the clock) for 5 s, then confirm. The touches are
 * only watched, never consumed, so whatever is under the finger still works.
 */
@Composable
fun KioskExitCorner(enabled: Boolean, onExit: () -> Unit, content: @Composable () -> Unit) {
    var asking by remember { mutableStateOf(false) }
    Box(
        Modifier.fillMaxSize().pointerInput(enabled) {
            if (!enabled) return@pointerInput
            val corner = 120.dp.toPx()
            val slop = viewConfiguration.touchSlop
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                if (down.position.x < size.width - corner || down.position.y > corner) return@awaitEachGesture
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
        AlertDialog(
            onDismissRequest = { asking = false },
            containerColor = PadboardColors.Surface2,
            title = { Text(stringResource(R.string.kiosk_exit_title), color = PadboardColors.Text) },
            text = { Text(stringResource(R.string.kiosk_exit_body), color = PadboardColors.TextMuted2) },
            confirmButton = {
                TextButton(onClick = {
                    asking = false
                    onExit()
                }) { Text(stringResource(R.string.kiosk_exit_confirm), color = PadboardColors.Red) }
            },
            dismissButton = {
                TextButton(onClick = { asking = false }) {
                    Text(stringResource(R.string.cancel), color = PadboardColors.Text)
                }
            }
        )
    }
}
