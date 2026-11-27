package io.github.hugoambiehl.padboard.ui.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType

/**
 * The PC refused the USB connection because it could not check that it came
 * from adb: said on every screen without a PC, never a silent search. Placed
 * at the bottom centre, away from the kiosk corner.
 */
@Composable
fun UsbRefusedNotice(modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(16.dp)
    Text(
        stringResource(R.string.usb_refused),
        style = PadboardType.Body,
        color = PadboardColors.Text,
        modifier =
        modifier
            .padding(bottom = 32.dp)
            .widthIn(max = 640.dp)
            .background(PadboardColors.Orange.copy(alpha = 0.16f), shape)
            .border(1.dp, PadboardColors.Orange.copy(alpha = 0.4f), shape)
            .padding(horizontal = 20.dp, vertical = 14.dp)
    )
}
