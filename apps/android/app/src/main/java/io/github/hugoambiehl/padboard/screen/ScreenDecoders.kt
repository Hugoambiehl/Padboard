package io.github.hugoambiehl.padboard.screen

import android.content.Context
import android.media.MediaCodecList
import android.media.MediaFormat
import android.util.DisplayMetrics
import android.view.WindowManager
import io.github.hugoambiehl.padboard.protocol.ScreenCapability
import io.github.hugoambiehl.padboard.protocol.ScreenDecoder
import io.github.hugoambiehl.padboard.protocol.VideoCodec
import java.util.logging.Logger

/** What the tablet can show as a second screen, sent in `hello` (#20). */
object ScreenDecoders {
    private val log: Logger = Logger.getLogger("ScreenDecoders")

    /** MIME type of each codec, H.264 first (the PC picks the first it encodes). */
    val mimes = listOf(
        VideoCodec.H264 to MediaFormat.MIMETYPE_VIDEO_AVC,
        VideoCodec.Vp8 to MediaFormat.MIMETYPE_VIDEO_VP8
    )

    fun mime(codec: VideoCodec): String = mimes.first { it.first == codec }.second

    /** Hardware decoders and the screen's size; null without any decoder. */
    fun capability(context: Context): ScreenCapability? {
        val decoders =
            runCatching {
                val infos = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos
                mimes.mapNotNull { (codec, mime) ->
                    val info =
                        infos.firstOrNull { info ->
                            !info.isEncoder &&
                                info.supportedTypes.any { it.equals(mime, ignoreCase = true) } &&
                                !software(info.name)
                        } ?: return@mapNotNull null
                    val video = info.getCapabilitiesForType(mime).videoCapabilities ?: return@mapNotNull null
                    // Limits in landscape: the stream is landscape.
                    val w = video.supportedWidths.upper
                    val h = video.supportedHeights.upper
                    ScreenDecoder(codec, maxOf(w, h).toUInt(), minOf(w, h).toUInt())
                }
            }.getOrElse {
                log.warning("cannot list the decoders: $it")
                emptyList()
            }
        if (decoders.isEmpty()) return null
        val metrics = DisplayMetrics()
        // The whole screen, system bars included (they are hidden).
        @Suppress("DEPRECATION")
        context.getSystemService(WindowManager::class.java).defaultDisplay.getRealMetrics(metrics)
        return ScreenCapability(
            displayWidth = maxOf(metrics.widthPixels, metrics.heightPixels).toUInt(),
            displayHeight = minOf(metrics.widthPixels, metrics.heightPixels).toUInt(),
            decoders = decoders,
            touch = false
        )
    }

    /** Android's software decoders: too slow on the SM-T580. */
    private fun software(name: String) = name.startsWith("OMX.google.") || name.startsWith("c2.android.")
}
