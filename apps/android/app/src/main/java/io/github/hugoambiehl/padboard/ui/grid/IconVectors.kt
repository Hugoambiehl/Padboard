package io.github.hugoambiehl.padboard.ui.grid

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.dp
import java.util.logging.Level
import java.util.logging.Logger

/**
 * Button icons sent by the desktop (protocol `ButtonIcon`): SVG path data in
 * Lucide's 24×24 grid, drawn as 2-unit round strokes. Drawn in white and
 * tinted by the tile, so one vector serves every color.
 *
 * Vectors are cached by their paths: a layout update carrying the same icon
 * reuses the same vector (and its rasterized painter), so nothing flickers.
 */
object IconVectors {
    private const val MAX_ENTRIES = 256
    private val log = Logger.getLogger("IconVectors")

    // Access-ordered map used as an LRU cache. Only touched from the UI thread,
    // synchronized anyway since it is cheap.
    private val cache =
        object : LinkedHashMap<List<String>, ImageVector?>(MAX_ENTRIES, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<List<String>, ImageVector?>) =
                size > MAX_ENTRIES
        }

    /** The vector for these paths, or `null` if they cannot be parsed (the tile shows the initial). */
    fun get(paths: List<String>): ImageVector? = synchronized(cache) {
        if (cache.containsKey(paths)) return cache[paths]
        build(paths).also { cache[paths] = it }
    }

    internal fun build(paths: List<String>): ImageVector? {
        if (paths.isEmpty()) return null
        return try {
            val builder =
                ImageVector.Builder(
                    defaultWidth = 24.dp,
                    defaultHeight = 24.dp,
                    viewportWidth = 24f,
                    viewportHeight = 24f
                )
            for (d in paths) {
                builder.addPath(
                    pathData = PathParser().parsePathString(d).toNodes(),
                    fill = null,
                    stroke = SolidColor(Color.White),
                    strokeLineWidth = 2f,
                    strokeLineCap = StrokeCap.Round,
                    strokeLineJoin = StrokeJoin.Round
                )
            }
            builder.build()
        } catch (e: IllegalArgumentException) {
            // The desktop validates paths; this is only a safety net.
            log.log(Level.WARNING, "unreadable icon path", e)
            null
        }
    }
}
