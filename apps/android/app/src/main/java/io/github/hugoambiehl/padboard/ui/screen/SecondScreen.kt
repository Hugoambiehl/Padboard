package io.github.hugoambiehl.padboard.ui.screen

import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.ScreenOffer
import io.github.hugoambiehl.padboard.protocol.ScreenStopReason
import io.github.hugoambiehl.padboard.screen.ScreenPlayer
import kotlinx.coroutines.delay

/** How long the way out is shown when the second screen opens. */
private const val HINT_MS = 4000L

/**
 * The second screen (#20): the PC's picture over the whole tablet, black
 * around it if its shape differs. Touches do nothing yet (no touch as
 * mouse). [onEnd] when the stream ends by itself (null: closed by the PC)
 * or the surface goes away.
 */
@Composable
fun SecondScreen(offer: ScreenOffer, onEnd: (ScreenStopReason?) -> Unit) {
    val end by rememberUpdatedState(onEnd)
    var hint by remember(offer) { mutableStateOf(true) }
    LaunchedEffect(offer) {
        delay(HINT_MS)
        hint = false
    }
    val hintAlpha by animateFloatAsState(if (hint) 1f else 0f, tween(durationMillis = 400), label = "hint")
    Box(Modifier.fillMaxSize().background(Color.Black), contentAlignment = Alignment.Center) {
        val ratio = offer.payload.width.toFloat() / offer.payload.height.toFloat()
        AndroidView(
            factory = { context ->
                SurfaceView(context).apply {
                    keepScreenOn = true
                    holder.addCallback(
                        object : SurfaceHolder.Callback {
                            var player: ScreenPlayer? = null

                            override fun surfaceCreated(holder: SurfaceHolder) {
                                player = ScreenPlayer(offer, holder.surface) { reason -> post { end(reason) } }
                            }

                            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) =
                                Unit

                            override fun surfaceDestroyed(holder: SurfaceHolder) {
                                val stopped = player != null
                                player?.close()
                                player = null
                                // The app left the front (no kiosk mode): the
                                // stream cannot go on without its surface.
                                if (stopped) end(ScreenStopReason.StreamFailed)
                            }
                        }
                    )
                }
            },
            modifier = Modifier.aspectRatio(ratio)
        )
        if (hintAlpha > 0f) {
            Text(
                text = stringResource(R.string.second_screen_exit_hint),
                color = Color.White,
                style = MaterialTheme.typography.bodyLarge,
                modifier =
                Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = 24.dp)
                    .graphicsLayer { alpha = hintAlpha }
                    .background(Color(0xCC17181D), RoundedCornerShape(12.dp))
                    .padding(horizontal = 20.dp, vertical = 10.dp)
            )
        }
    }
}
