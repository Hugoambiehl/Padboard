package io.github.hugoambiehl.padboard.kiosk

import android.util.Log
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.github.hugoambiehl.padboard.connection.DiscoveredPc
import io.github.hugoambiehl.padboard.connection.LinkState
import io.github.hugoambiehl.padboard.connection.ScreenOffer
import io.github.hugoambiehl.padboard.connection.Session
import io.github.hugoambiehl.padboard.connection.Transport
import io.github.hugoambiehl.padboard.connection.UpdateProgress
import io.github.hugoambiehl.padboard.protocol.ButtonView
import io.github.hugoambiehl.padboard.protocol.Category
import io.github.hugoambiehl.padboard.protocol.Folder
import io.github.hugoambiehl.padboard.protocol.GridSize
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.NavigateTarget
import io.github.hugoambiehl.padboard.protocol.Page
import io.github.hugoambiehl.padboard.protocol.ProfileInfo
import io.github.hugoambiehl.padboard.protocol.ScreenOfferPayload
import io.github.hugoambiehl.padboard.protocol.VideoCodec
import io.github.hugoambiehl.padboard.ui.about.AboutScreen
import io.github.hugoambiehl.padboard.ui.connect.IncompatibleScreen
import io.github.hugoambiehl.padboard.ui.connect.PinScreen
import io.github.hugoambiehl.padboard.ui.connect.SearchingScreen
import io.github.hugoambiehl.padboard.ui.connect.SetupScreen
import io.github.hugoambiehl.padboard.ui.connect.UpdateOnlyScreen
import io.github.hugoambiehl.padboard.ui.grid.GridScreen
import io.github.hugoambiehl.padboard.ui.grid.GridStatus
import io.github.hugoambiehl.padboard.ui.grid.PageMemory
import io.github.hugoambiehl.padboard.ui.screen.SecondScreen
import io.github.hugoambiehl.padboard.ui.theme.PadboardTheme
import io.github.hugoambiehl.padboard.ui.update.UpdateOverlay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.emptyFlow
import okhttp3.OkHttpClient
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The hidden corner that leaves kiosk mode ([KIOSK_EXIT_CORNER], bottom
 * left) must hold nothing on any screen: every screen is drawn at the
 * SM-T580's size (1280 × 800 dp), and no element of it (button, text, icon)
 * may reach into the corner. Run on a throwaway emulator:
 *
 *   scripts/tablet-ui-tests.sh
 */
@RunWith(AndroidJUnit4::class)
class KioskCornerTest {
    @get:Rule val compose = createComposeRule()

    /**
     * Every screen, one after the other in the same activity (an emulator
     * with a buggy graphics stack crashed while switching activities).
     */
    private val screens: List<Pair<String, @Composable () -> Unit>> =
        listOf(
            "setup" to { SetupScreen(PCS, emptyFlow(), onPick = {}, onManual = {}, onBack = {}, onAbout = {}) },
            "pin" to {
                PinScreen(LinkState.Pairing("PC-BUREAU", "ABCD-EFGH-IJKL", attemptsLeft = 3), onSubmit = {
                }, onCancel = {})
            },
            "searching" to { SearchingScreen("PC-BUREAU", onForget = {}, attempt = 3, onAbout = {}) },
            "incompatible" to { IncompatibleScreen(onRetry = {}) },
            "update only" to { UpdateOnlyScreen("PC-BUREAU", pcIsNewer = true) },
            "about" to { AboutScreen(onBack = {}) },
            "update" to { UpdateOverlay(UpdateProgress.Receiving("0.3.0", 10, 100), onDismiss = {}) },
            "grid" to { Grid(CONNECTED) },
            "folder" to {
                Grid(
                    CONNECTED,
                    MutableSharedFlow<NavigatePayload>(replay = 1).apply {
                        tryEmit(NavigatePayload(NavigateTarget.Folder, "f"))
                    }
                )
            },
            "second screen" to {
                // No PC behind it: the player fails quietly, the screen stays.
                val nowhere = "ws://127.0.0.1:9"
                SecondScreen(
                    ScreenOffer(
                        ScreenOfferPayload("ticket", VideoCodec.H264, 1728u, 1080u, 60u),
                        Session.open(OkHttpClient(), "$nowhere/"),
                        OkHttpClient(),
                        nowhere
                    ),
                    onEnd = {}
                )
            },
            "disconnected" to {
                Grid(
                    GridStatus.Reconnecting("PC-BUREAU", wifi = true, retryInMs = 2_000, attempt = 4, announced = true)
                )
            }
        )

    @Test
    fun noScreenHasAnythingInTheHiddenCorner() {
        var cornerPx = 0f
        var shown by mutableIntStateOf(0)
        // Driven by hand: spinners (setup, searching) never let the clock settle.
        compose.mainClock.autoAdvance = false
        compose.setContent {
            cornerPx = with(LocalDensity.current) { KIOSK_EXIT_CORNER.toPx() }
            PadboardTheme {
                Box(Modifier.requiredSize(1280.dp, 800.dp)) {
                    // Keyed: each screen starts afresh, like after a state change in the app.
                    key(shown) { screens[shown].second() }
                }
            }
        }
        val problems = mutableListOf<String>()
        screens.forEachIndexed { index, (name, _) ->
            shown = index
            // Past the grid's appearance and the disconnected card's grace period.
            compose.mainClock.advanceTimeBy(2_500)
            val root = compose.onRoot(useUnmergedTree = true).fetchSemanticsNode()
            val screen = root.boundsInRoot
            val corner = Rect(0f, screen.bottom - cornerPx, cornerPx, screen.bottom)
            var nodes = 0
            fun visit(node: SemanticsNode) {
                nodes++
                val b = node.boundsInRoot
                val wholeScreen = b.width >= screen.width && b.height >= screen.height
                if (!wholeScreen && b.width > 0 && b.height > 0 && b.overlaps(corner)) {
                    problems += "$name: ${node.config} at $b"
                }
                node.children.forEach(::visit)
            }
            visit(root)
            Log.i("KioskCornerTest", "$name: $nodes elements, corner $corner free")
        }
        assertTrue("in the hidden corner: $problems", problems.isEmpty())
    }

    @Composable
    private fun Grid(status: GridStatus, navigations: MutableSharedFlow<NavigatePayload> = MutableSharedFlow()) {
        GridScreen(
            layout = LAYOUT,
            pageMemory = PageMemory(),
            status = status,
            actionFailures = emptyFlow(),
            navigations = navigations,
            onPress = {},
            onRelease = {},
            onPairWifi = {},
            onForget = {},
            onAbout = {},
            onRetry = {}
        )
    }

    private companion object {
        val PCS =
            listOf(
                DiscoveredPc("a", "PC-BUREAU", 5, listOf("192.168.1.20"), 47322),
                DiscoveredPc("b", "PORTABLE", 5, listOf("192.168.1.21"), 47322)
            )
        val CONNECTED = GridStatus.Connected("PC-BUREAU", Transport.Wifi, latencyMs = 12, paired = true)

        /** Every cell filled, so the corner tiles are there. */
        fun full(prefix: String) = (0u until 3u).flatMap { row ->
            (0u until 5u).map { col ->
                ButtonView(
                    "$prefix$row$col",
                    row,
                    col,
                    "Bouton $row$col",
                    subtitle = "Sous-titre",
                    category = Category.Violet,
                    active = false,
                    unavailable = false
                )
            }
        }
        val LAYOUT =
            Layout(
                ProfileInfo("p", "Streaming"),
                GridSize(5u, 3u),
                listOf(Page("1", full("a")), Page("2", full("b"))),
                listOf(Folder("f", "Dossier OBS", full("f")))
            )
    }
}
