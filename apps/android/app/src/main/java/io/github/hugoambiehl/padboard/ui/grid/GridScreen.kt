package io.github.hugoambiehl.padboard.ui.grid

import android.text.format.DateFormat
import androidx.compose.animation.core.animateFloatAsState
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.ServerAddress
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import java.util.Date
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow

private const val ERROR_BANNER_MS = 3_000L

/** Main screen: top bar and the button grid of the current page (Main.dc.html). */
@Composable
fun GridScreen(
    layout: Layout?,
    serverName: String,
    address: ServerAddress,
    actionFailures: Flow<String>,
    onPress: (String) -> Unit,
    onRelease: (String) -> Unit,
    onDisconnect: () -> Unit
) {
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
                serverName = serverName,
                address = address,
                onDisconnect = onDisconnect
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
                ButtonGrid(
                    cols = layout.grid.cols.toInt(),
                    rows = layout.grid.rows.toInt(),
                    buttons = page.buttons,
                    onPress = onPress,
                    onRelease = onRelease,
                    modifier = Modifier.fillMaxWidth().weight(1f)
                )
            }
        }
        ErrorBanner(visible = errorVisible, modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 16.dp))
    }
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
private fun TopBar(
    profileName: String,
    pageCount: Int,
    pageIndex: Int,
    serverName: String,
    address: ServerAddress,
    onDisconnect: () -> Unit
) {
    Box(Modifier.fillMaxWidth().height(48.dp)) {
        ProfileChip(profileName, Modifier.align(Alignment.CenterStart))
        PageDots(pageCount, pageIndex, Modifier.align(Alignment.Center))
        Row(
            modifier = Modifier.align(Alignment.CenterEnd),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            ConnectionPill(serverName, address, onDisconnect)
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
private fun ConnectionPill(serverName: String, address: ServerAddress, onDisconnect: () -> Unit) {
    val transport = stringResource(if (address.isUsb) R.string.transport_usb else R.string.transport_wifi)
    val description = stringResource(R.string.connection_pill_description, serverName, transport)
    Row(
        modifier =
        Modifier
            .clip(CircleShape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, CircleShape)
            .clickable(onClick = onDisconnect)
            .semantics { contentDescription = description }
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(PadboardColors.Teal))
        Text(serverName, style = PadboardType.Pill, color = PadboardColors.TextPill, maxLines = 1)
        Text("· $transport", style = PadboardType.Pill, color = PadboardColors.TextMuted)
    }
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
