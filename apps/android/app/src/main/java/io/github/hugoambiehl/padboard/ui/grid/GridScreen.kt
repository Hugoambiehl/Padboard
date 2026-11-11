package io.github.hugoambiehl.padboard.ui.grid

import android.text.format.DateFormat
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
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
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.NavigateTarget
import io.github.hugoambiehl.padboard.ui.connect.ForgetDialog
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import java.util.Date
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch

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

    /** The grid stays visible but inactive, under [DisconnectedCard]. */
    data class Reconnecting(
        override val serverName: String,
        /** Paired over Wi-Fi (else only the cable can bring the PC back). */
        val wifi: Boolean,
        val retryInMs: Long,
        val attempt: Int
    ) : GridStatus
}

/** Main screen: top bar and the button grid of the current page (Main.dc.html). */
@Composable
fun GridScreen(
    layout: Layout?,
    pageMemory: PageMemory,
    status: GridStatus,
    actionFailures: Flow<String>,
    navigations: Flow<NavigatePayload>,
    onPress: (String) -> Unit,
    onRelease: (String) -> Unit,
    onPairWifi: () -> Unit,
    onForget: () -> Unit,
    onRetry: () -> Unit
) {
    var menu by remember { mutableStateOf(false) }
    val pages = layout?.pages.orEmpty()
    val currentPages by rememberUpdatedState(pages)
    val currentLayout by rememberUpdatedState(layout)
    // The page in front of the user lives in `pageMemory` (ViewModel), so this
    // screen leaving the composition (reconnecting, pairing screen) loses
    // nothing. Layout updates are applied during composition with
    // requestScrollToPage, so a new layout never shows a wrong page for a frame.
    val pagerState =
        rememberPagerState(initialPage = layout?.let(pageMemory::pageFor) ?: 0) { currentPages.size }
    val tracked = remember { arrayOfNulls<Layout>(1) }
    if (layout != null && tracked[0] !== layout) {
        val target = pageMemory.pageFor(layout)
        tracked[0] = layout
        pageMemory.onShown(layout, target)
        if (target != pagerState.currentPage) pagerState.requestScrollToPage(target)
    }
    LaunchedEffect(pagerState) {
        snapshotFlow { pagerState.currentPage }.collect { index ->
            currentLayout?.let { pageMemory.onShown(it, index) }
        }
    }
    // Navigation buttons: the PC says where to go, relative moves are resolved here.
    // The collector never waits for an animation: one that cannot finish
    // (pages hidden by a folder, screen leaving) would block every later
    // request. Each scroll runs on its own and the next one replaces it.
    val scope = rememberCoroutineScope()
    val scroll = remember { arrayOfNulls<Job>(1) }
    LaunchedEffect(navigations) {
        navigations.collect { request ->
            val current = currentLayout ?: return@collect
            val folderShown = pageMemory.folderFor(current) != null
            val target = pageMemory.navigate(current, request, pagerState.currentPage) ?: return@collect
            scroll[0]?.cancel()
            if (folderShown) {
                // The pages come back with this request: no animation to see.
                pagerState.requestScrollToPage(target)
            } else {
                scroll[0] = scope.launch { pagerState.animateScrollToPage(target) }
            }
        }
    }
    val folder = layout?.let(pageMemory::folderFor)
    val back = { layout?.let { pageMemory.navigate(it, NavigatePayload(NavigateTarget.Back), 0) } }
    BackHandler(enabled = folder != null) { back() }
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
                folderName = folder?.name,
                onBack = { back() },
                pageCount = pages.size,
                pagerState = pagerState,
                status = status,
                onPill = { menu = true }
            )
            if (layout == null || pages.isEmpty()) {
                Box(Modifier.fillMaxWidth().weight(1f), contentAlignment = Alignment.Center) {
                    Text(
                        stringResource(R.string.layout_waiting),
                        style = PadboardType.Small,
                        color = PadboardColors.TextMuted
                    )
                }
            } else {
                val live = status is GridStatus.Connected
                // Dimmed and explained only after a grace period: a USB ↔ Wi-Fi
                // hand-over or a blip should not flash the screen.
                var lost by remember { mutableStateOf(false) }
                LaunchedEffect(live) {
                    lost = false
                    if (!live) {
                        delay(DISCONNECTED_GRACE_MS)
                        lost = true
                    }
                }
                var lastReconnecting by remember { mutableStateOf<GridStatus.Reconnecting?>(null) }
                if (status is GridStatus.Reconnecting) lastReconnecting = status
                val dim by animateFloatAsState(if (lost) 0.3f else 1f, tween(200), label = "gridDim")
                Box(Modifier.fillMaxWidth().weight(1f)) {
                    // The pager always stays composed (recomposing it and its
                    // pages when a folder closes made frames drop on the
                    // tablet); an open folder fades in over it, alpha only.
                    val folderAlpha by animateFloatAsState(
                        if (folder != null) 1f else 0f,
                        tween(150),
                        label = "folder"
                    )
                    // The last folder shown, kept while it fades out (and after).
                    var shownFolder by remember { mutableStateOf(folder) }
                    if (folder != null) shownFolder = folder
                    val folderOpen by rememberUpdatedState(folder != null)
                    val overlayVisible by remember { derivedStateOf { folderOpen || folderAlpha > 0f } }
                    Box(Modifier.fillMaxSize().graphicsLayer { alpha = dim }) {
                        HorizontalPager(
                            state = pagerState,
                            key = { currentPages.getOrNull(it)?.id ?: it },
                            // Neighbours composed ahead, so a swipe starts without a hitch.
                            beyondViewportPageCount = 1,
                            pageSpacing = 40.dp,
                            // No userScrollEnabled toggle: the folder layer above takes
                            // every touch while open, and toggling recomposed the pager.
                            modifier = Modifier.fillMaxSize().graphicsLayer { alpha = 1f - folderAlpha }
                        ) { index ->
                            val page = currentPages.getOrNull(index)
                            if (page != null) {
                                ButtonGrid(
                                    cols = layout.grid.cols.toInt(),
                                    rows = layout.grid.rows.toInt(),
                                    buttons = page.buttons,
                                    onPress = onPress,
                                    onRelease = onRelease,
                                    modifier = Modifier.fillMaxSize().then(CachedLayer)
                                )
                            }
                        }
                        val overlay = shownFolder?.let { f -> layout.folders.firstOrNull { it.id == f.id } }
                        if (overlay != null) {
                            // Kept composed once opened (composing it on each open dropped
                            // frames); while closed it is not placed: neither drawn nor touchable.
                            Box(
                                Modifier
                                    .fillMaxSize()
                                    .layout { measurable, constraints ->
                                        val placeable = measurable.measure(constraints)
                                        layout(placeable.width, placeable.height) {
                                            if (overlayVisible) placeable.place(0, 0)
                                        }
                                    }.graphicsLayer { alpha = folderAlpha }
                            ) {
                                // Touches on the folder's empty cells must not reach the pages below.
                                Box(
                                    Modifier.matchParentSize().pointerInput(Unit) {
                                        awaitPointerEventScope {
                                            while (true) awaitPointerEvent().changes.forEach { it.consume() }
                                        }
                                    }
                                )
                                ButtonGrid(
                                    cols = layout.grid.cols.toInt(),
                                    rows = layout.grid.rows.toInt(),
                                    buttons = overlay.buttons,
                                    onPress = onPress,
                                    onRelease = onRelease,
                                    modifier = Modifier.fillMaxSize().then(CachedLayer)
                                )
                            }
                        }
                    }
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
                    lastReconnecting?.let {
                        DisconnectedCard(
                            status = it,
                            visible = lost,
                            onRetry = onRetry,
                            onPairWifi = onPairWifi,
                            modifier = Modifier.align(Alignment.Center)
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
private fun TopBar(
    profileName: String,
    folderName: String?,
    onBack: () -> Unit,
    pageCount: Int,
    pagerState: PagerState,
    status: GridStatus,
    onPill: () -> Unit
) {
    Box(Modifier.fillMaxWidth().height(48.dp)) {
        if (folderName != null) {
            BackChip(folderName, onBack, Modifier.align(Alignment.CenterStart))
        } else {
            ProfileChip(profileName, Modifier.align(Alignment.CenterStart))
            PageDots(pageCount, pagerState, Modifier.align(Alignment.Center))
        }
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

/** In a folder: back to the pages (or the enclosing folder). */
@Composable
private fun BackChip(folderName: String, onBack: () -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(14.dp)
    val description = stringResource(R.string.folder_back, folderName)
    Row(
        modifier =
        modifier
            .clip(shape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
            .clickable(onClick = onBack)
            .semantics { contentDescription = description }
            .padding(start = 7.dp, top = 7.dp, bottom = 7.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Box(
            Modifier.size(34.dp).clip(RoundedCornerShape(10.dp)).background(PadboardColors.SurfaceActive),
            contentAlignment = Alignment.Center
        ) {
            Icon(painterResource(R.drawable.ic_chevron_left), null, Modifier.size(18.dp), tint = PadboardColors.Text)
        }
        Text(folderName, style = PadboardType.Profile, color = PadboardColors.Text)
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

/**
 * A grid drawn once into an offscreen layer and reused: a page sliding during
 * a swipe or a folder fading in only moves or fades that texture, instead of
 * redrawing every tile on each frame (measured on the SM-T580: ~45 ms per
 * frame without it). Only transform and alpha change, as the tablet
 * animation rule requires.
 */
private val CachedLayer = Modifier.graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }

private val DotSize = 8.dp
private val DotGap = 8.dp
private val ActiveDotWidth = 26.dp

/**
 * Page indicator (Main.dc.html): the current page is a wider, lighter pill.
 * It follows the finger during a swipe. Everything happens in the draw phase
 * (the pager position is read inside the Canvas), so a swipe never triggers
 * a layout pass; hidden for a single page.
 */
@Composable
private fun PageDots(count: Int, pagerState: PagerState, modifier: Modifier = Modifier) {
    if (count <= 1) return
    val description = stringResource(R.string.page_indicator_description, pagerState.currentPage + 1, count)
    val width = DotSize * count + DotGap * (count - 1) + (ActiveDotWidth - DotSize)
    Canvas(
        modifier
            .width(width)
            .height(DotSize)
            .semantics { contentDescription = description }
    ) {
        val position = pagerState.currentPage + pagerState.currentPageOffsetFraction
        val dot = DotSize.toPx()
        val extra = (ActiveDotWidth - DotSize).toPx()
        var x = 0f
        repeat(count) { i ->
            // 1 for the current page, sliding to the next one during a swipe.
            val weight = (1f - kotlin.math.abs(i - position)).coerceIn(0f, 1f)
            val w = dot + extra * weight
            drawRoundRect(
                color = lerp(PadboardColors.DotInactive, PadboardColors.Text, weight),
                topLeft = Offset(x, 0f),
                size = Size(w, dot),
                cornerRadius = CornerRadius(dot / 2)
            )
            x += w + DotGap.toPx()
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
