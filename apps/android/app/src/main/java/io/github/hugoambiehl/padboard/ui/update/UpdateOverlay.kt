package io.github.hugoambiehl.padboard.ui.update

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.UpdateProgress
import io.github.hugoambiehl.padboard.protocol.UpdateOutcome
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType

/**
 * An update sent by the PC, over everything: received, installing (the
 * app restarts by itself), or why it failed. Touches do not reach the grid
 * meanwhile.
 */
@Composable
fun UpdateOverlay(progress: UpdateProgress, onDismiss: () -> Unit) {
    Box(
        modifier =
        Modifier
            .fillMaxSize()
            .background(Color.Black.copy(alpha = 0.72f))
            // Swallows every touch: no button press while the app is replaced.
            .pointerInput(Unit) {
                awaitPointerEventScope { while (true) awaitPointerEvent().changes.forEach { it.consume() } }
            },
        contentAlignment = Alignment.Center
    ) {
        val shape = RoundedCornerShape(22.dp)
        Column(
            modifier =
            Modifier
                .widthIn(max = 520.dp)
                .clip(shape)
                .background(PadboardColors.Surface2)
                .border(1.dp, PadboardColors.Border, shape)
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(stringResource(R.string.update_title), style = PadboardType.CardTitle, color = PadboardColors.Text)
            when (progress) {
                is UpdateProgress.Receiving -> {
                    val fraction = if (progress.total > 0) progress.received.toFloat() / progress.total else 0f
                    Text(
                        stringResource(R.string.update_receiving, progress.version, (fraction * 100).toInt()),
                        style = PadboardType.Body,
                        color = PadboardColors.TextMuted2
                    )
                    LinearProgressIndicator(
                        progress = { fraction },
                        modifier = Modifier.fillMaxWidth(),
                        color = PadboardColors.Accent,
                        trackColor = PadboardColors.SurfaceActive
                    )
                }
                is UpdateProgress.Installing ->
                    Text(
                        stringResource(R.string.update_installing, progress.version),
                        style = PadboardType.Body,
                        color = PadboardColors.TextMuted2
                    )
                is UpdateProgress.Failed -> {
                    Text(
                        stringResource(
                            R.string.update_failed,
                            progress.version,
                            stringResource(reason(progress.outcome))
                        ),
                        style = PadboardType.Body,
                        color = PadboardColors.Text
                    )
                    Button(
                        onClick = onDismiss,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = PadboardColors.Accent,
                            contentColor = PadboardColors.Background
                        )
                    ) { Text(stringResource(R.string.update_ok)) }
                }
            }
        }
    }
}

private fun reason(outcome: UpdateOutcome): Int = when (outcome) {
    UpdateOutcome.NotDeviceOwner -> R.string.update_reason_not_device_owner
    UpdateOutcome.NotNewer -> R.string.update_reason_not_newer
    UpdateOutcome.WrongPackage -> R.string.update_reason_wrong_package
    UpdateOutcome.BadCertificate -> R.string.update_reason_bad_certificate
    UpdateOutcome.BadHash -> R.string.update_reason_bad_hash
    UpdateOutcome.TransferFailed -> R.string.update_reason_transfer_failed
    UpdateOutcome.Busy -> R.string.update_reason_busy
    UpdateOutcome.InstallFailed, UpdateOutcome.Installing -> R.string.update_reason_install_failed
}
