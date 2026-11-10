package io.github.hugoambiehl.padboard.ui.grid

import android.text.format.DateFormat
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.Transport
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.ui.connect.ForgetDialog
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import java.util.Date
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow

private const val ERROR_BANNER_MS = 3_000L

/** What the top bar shows about the link. */
sealed interface GridStatus {
    val serverName: String

    data class Connected(
        override val serverName: String,
        val transport: Transport,
        val latencyMs: Int?,
        /** Paired over Wi-Fi with this PC. */
        val paired: Boolean
    ) : GridStatus

    /** The grid stays visible but inactive. */
    data class Reconnecting(override val serverName: String) : GridStatus
}

/** Main screen: top bar and the button grid of the current page (Main.dc.html). */
@Composable
fun GridScreen(
    layout: Layout?,
    status: GridStatus,
    actionFailures: Flow<String>,
    onPress: (String) -> Unit,
    onRelease: (String) -> Unit,
    onPairWifi: () -> Unit,
    onForget: () -> Unit
) {
    var menu by remember { mutableStateOf(false) }
    // Phase 1 shows the first page; page swipes come in phase 4.
    val pageIndex = 0
    var errorVisible by remember { mutableStateOf(false) }
    LaunchedEffect(actionFailures) {
        actionFailures.collect {
            errorVisible = true
            delay(ERROR_BANNER_MS)
            errorVisible = false
        }
    }

    Box(Modifier.fillMaxSize().background(PadboardColors.Background)) {
        Column(
            modifier = Modifier.fillMaxSize().padding(start = 40.dp, end = 40.dp, top = 28.dp, bottom = 40.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            TopBar(
                profileName = layout?.profile?.name.orEmpty(),
                pageCount = layout?.pages?.size ?: 0,
                pageIndex = pageIndex,
                status = status,
                onPill = { menu = true }
            )
            val page = layout?.pages?.getOrNull(pageIndex)
            if (layout == null || page == null) {
                Box(Modifier.fillMaxWidth().weight(1f), contentAlignment = Alignment.Center) {
                    Text(
                        stringResource(R.string.layout_waiting),
                        style = PadboardType.Small,
                        color = PadboardColors.TextMuted
                    )
                }
            } else {
                val live = status is GridStatus.Connected
                val dim by animateFloatAsState(if (live) 1f else 0.4f, tween(200), label = "gridDim")
                Box(Modifier.fillMaxWidth().weight(1f)) {
                    ButtonGrid(
                        cols = layout.grid.cols.toInt(),
                        rows = layout.grid.rows.toInt(),
                        buttons = page.buttons,
                        onPress = onPress,
                        onRelease = onRelease,
                        modifier = Modifier.fillMaxSize().graphicsLayer { alpha = dim }
                    )
                    if (!live) {
                        // Presses would go nowhere while reconnecting: swallow them.
                        Box(
                            Modifier.matchParentSize().pointerInput(Unit) {
                                awaitPointerEventScope {
                                    while (true) awaitPointerEvent().changes.forEach { it.consume() }
                                }
                            }
                        )
                    }
                }
            }
        }
        ErrorBanner(visible = errorVisible, modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 16.dp))
    }
    if (menu) {
        LinkMenu(status, onPairWifi = onPairWifi, onForget = onForget, onDismiss = { menu = false })
    }
}

/** Opened from the connection pill. */
@Composable
private fun LinkMenu(status: GridStatus, onPairWifi: () -> Unit, onForget: () -> Unit, onDismiss: () -> Unit) {
    val usbOnly = status is GridStatus.Connected && !status.paired
    if (!usbOnly) {
        ForgetDialog(status.serverName, onConfirm = onForget, onDismiss = onDismiss)
        return
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = PadboardColors.Surface2,
        title = { Text(status.serverName, color = PadboardColors.Text) },
        text = { Text(stringResource(R.string.usb_only_body), color = PadboardColors.TextMuted2) },
        confirmButton = {
            TextButton(onClick = {
                onDismiss()
                onPairWifi()
            }) { Text(stringResource(R.string.pair_wifi), color = PadboardColors.Accent) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel), color = PadboardColors.Text) }
        }
    )
}

@Composable
private fun ButtonGrid(
    cols: Int,
    rows: Int,
    buttons: List<ButtonView>,
    onPress: (String) -> Unit,
    onRelease: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val byCell = remember(buttons) { buttons.associateBy { it.row.toInt() to it.col.toInt() } }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(20.dp)) {
        repeat(rows) { row ->
            Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                repeat(cols) { col ->
                    val button = byCell[row to col]
                    val cell = Modifier.weight(1f).fillMaxSize()
                    if (button == null) {
                        Spacer(cell)
                    } else {
                        ButtonTile(
                            button = button,
                            onPress = { onPress(button.id) },
                            onRelease = { onRelease(button.id) },
                            modifier = cell
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun TopBar(profileName: String, pageCount: Int, pageIndex: Int, status: GridStatus, onPill: () -> Unit) {
    Box(Modifier.fillMaxWidth().height(48.dp)) {
        ProfileChip(profileName, Modifier.align(Alignment.CenterStart))
        PageDots(pageCount, pageIndex, Modifier.align(Alignment.Center))
        Row(
            modifier = Modifier.align(Alignment.CenterEnd),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            ConnectionPill(status, onPill)
            Clock()
        }
    }
}

@Composable
private fun ProfileChip(name: String, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(14.dp)
    Row(
        modifier =
        modifier
            .clip(shape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
            .padding(start = 7.dp, top = 7.dp, bottom = 7.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Box(
            Modifier.size(34.dp).clip(RoundedCornerShape(10.dp)).background(PadboardColors.Accent),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                painterResource(R.drawable.ic_layout_grid),
                null,
                Modifier.size(18.dp),
                tint = PadboardColors.Background
            )
        }
        Text(name, style = PadboardType.Profile, color = PadboardColors.Text)
        Icon(painterResource(R.drawable.ic_chevron_down), null, Modifier.size(16.dp), tint = PadboardColors.TextMuted)
    }
}

@Composable
private fun PageDots(count: Int, current: Int, modifier: Modifier = Modifier) {
    if (count <= 0) return
    val description = stringResource(R.string.page_indicator_description, current + 1, count)
    Row(
        modifier = modifier.semantics { contentDescription = description },
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        repeat(count) { i ->
            val active = i == current
            Box(
                Modifier
                    .width(if (active) 26.dp else 8.dp)
                    .height(8.dp)
                    .clip(CircleShape)
                    .background(if (active) PadboardColors.Text else PadboardColors.DotInactive)
            )
        }
    }
}

@Composable
private fun ConnectionPill(status: GridStatus, onClick: () -> Unit) {
    val detail =
        when (status) {
            is GridStatus.Connected -> {
                val transport =
                    stringResource(
                        if (status.transport ==
                            Transport.Usb
                        ) {
                            R.string.transport_usb
                        } else {
                            R.string.transport_wifi
                        }
                    )
                status.latencyMs?.let { stringResource(R.string.pill_latency, transport, it) } ?: transport
            }
            is GridStatus.Reconnecting -> stringResource(R.string.pill_reconnecting)
        }
    val description = stringResource(R.string.connection_pill_description, status.serverName, detail)
    Row(
        modifier =
        Modifier
            .clip(CircleShape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, CircleShape)
            .clickable(onClick = onClick)
            .semantics { contentDescription = description }
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        StatusDot(connected = status is GridStatus.Connected)
        // Hostnames can be long ("My-Laptop-15-…"): keep the pill clear of the page dots.
        Text(
            status.serverName,
            style = PadboardType.Pill,
            color = PadboardColors.TextPill,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 240.dp)
        )
        Text("· $detail", style = PadboardType.Pill, color = PadboardColors.TextMuted)
    }
}

/** Teal when connected; orange and pulsing (alpha only) while reconnecting. */
@Composable
private fun StatusDot(connected: Boolean) {
    val pulse =
        if (connected) {
            1f
        } else {
            val transition = rememberInfiniteTransition(label = "pulse")
            val alpha by transition.animateFloat(
                initialValue = 1f,
                targetValue = 0.3f,
                animationSpec = infiniteRepeatable(tween(600), RepeatMode.Reverse),
                label = "pulseAlpha"
            )
            alpha
        }
    Box(
        Modifier
            .size(8.dp)
            .graphicsLayer { alpha = pulse }
            .clip(CircleShape)
            .background(if (connected) PadboardColors.Teal else PadboardColors.Orange)
    )
}

/** Current time in the user's 12/24 h preference, refreshed on each minute. */
@Composable
private fun Clock() {
    val context = LocalContext.current
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            now = System.currentTimeMillis()
            delay(60_000 - now % 60_000)
        }
    }
    val text = remember(now) { DateFormat.getTimeFormat(context).format(Date(now)) }
    Text(text, style = PadboardType.Clock, color = PadboardColors.Text)
}

@Composable
private fun ErrorBanner(visible: Boolean, modifier: Modifier = Modifier) {
    val alpha by animateFloatAsState(if (visible) 1f else 0f, tween(200), label = "errorBanner")
    val shape = RoundedCornerShape(14.dp)
    Text(
        text = stringResource(R.string.action_failed),
        style = PadboardType.Small,
        color = PadboardColors.Text,
        modifier =
        modifier
            .graphicsLayer { this.alpha = alpha }
            .clip(shape)
            .background(PadboardColors.Red.copy(alpha = 0.18f))
            .border(1.dp, PadboardColors.Red.copy(alpha = 0.5f), shape)
            .padding(horizontal = 16.dp, vertical = 10.dp)
    )
}
