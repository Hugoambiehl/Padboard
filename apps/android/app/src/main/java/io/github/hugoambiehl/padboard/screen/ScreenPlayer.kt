package io.github.hugoambiehl.padboard.screen

import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.view.Surface
import io.github.hugoambiehl.padboard.connection.ScreenOffer
import io.github.hugoambiehl.padboard.protocol.ScreenStopReason
import io.github.hugoambiehl.padboard.protocol.StreamFrame
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.logging.Level
import java.util.logging.Logger
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString

/**
 * Plays a second screen (#20) onto [surface]: opens the stream connection
 * of [offer], decodes its frames with the hardware decoder and acks each
 * one when it is shown (the PC sends at most two unacknowledged frames).
 *
 * Threads: OkHttp's (frames in), one feeding the decoder, one taking its
 * pictures. [onEnd] is called once, from any thread, when the stream ends
 * by itself: null when the PC closed it, else why it failed. Not called
 * after [close].
 */
class ScreenPlayer(
    private val offer: ScreenOffer,
    private val surface: Surface,
    private val onEnd: (ScreenStopReason?) -> Unit
) {
    private val frames = LinkedBlockingQueue<StreamFrame.Video>()
    private val open = AtomicBoolean(true)

    /** When each frame went into the decoder (capture µs → nanoTime). */
    private val queuedAt = ConcurrentHashMap<Long, Long>()
    private val socket: WebSocket = offer.open(Listener())
    private val feeder = Thread(::feed, "screen-decoder")

    init {
        feeder.start()
    }

    /** Stops everything (the second screen left); [onEnd] is not called. */
    fun close() {
        if (!open.getAndSet(false)) return
        socket.close(NORMAL_CLOSURE, null)
        feeder.interrupt()
    }

    private fun end(reason: ScreenStopReason?) {
        if (!open.getAndSet(false)) return
        log.info("second screen ended: ${reason ?: "closed by the PC"}")
        socket.cancel()
        feeder.interrupt()
        onEnd(reason)
    }

    private inner class Listener : WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            log.info("second screen stream open: $offer")
        }

        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
            val decoded = StreamFrame.decode(bytes.toByteArray())
            val frame = (decoded as? StreamFrame.Decoded.Frame)?.frame
            if (frame is StreamFrame.Video) {
                frames.put(frame)
            } else {
                log.warning("unexpected stream message: $decoded")
            }
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(NORMAL_CLOSURE, null)
            end(null)
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            // After close() or end(): the socket's own cancellation.
            if (!open.get()) return
            log.log(Level.WARNING, "second screen stream failed", t)
            end(ScreenStopReason.StreamFailed)
        }
    }

    private fun configure(): MediaCodec {
        val mime = ScreenDecoders.mime(offer.payload.codec)
        val width = offer.payload.width.toInt()
        val height = offer.payload.height.toInt()
        var lowLatency = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R
        while (true) {
            val codec = MediaCodec.createDecoderByType(mime)
            val format = MediaFormat.createVideoFormat(mime, width, height)
            if (lowLatency && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                format.setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
            }
            try {
                codec.configure(format, surface, null, 0)
                log.info("decoder ${codec.name}: $mime ${width}x$height, low latency $lowLatency")
                return codec
            } catch (e: IllegalArgumentException) {
                codec.release()
                // The SM-T580's Exynos decoders refuse KEY_LOW_LATENCY.
                if (!lowLatency) throw e
                lowLatency = false
            } catch (e: MediaCodec.CodecException) {
                codec.release()
                if (!lowLatency) throw e
                lowLatency = false
            }
        }
    }

    /** Feeds the decoder; a second thread shows its pictures and acks them. */
    private fun feed() {
        val codec =
            try {
                configure().also { it.start() }
            } catch (e: Exception) {
                log.log(Level.WARNING, "no decoder for the second screen", e)
                end(ScreenStopReason.DecoderFailed)
                return
            }
        val shower = Thread({ show(codec) }, "screen-display")
        shower.start()
        var keyFrameSeen = false
        try {
            while (open.get()) {
                val frame = frames.poll(POLL_MS, TimeUnit.MILLISECONDS) ?: continue
                if (!keyFrameSeen) {
                    // A decoder needs a key frame first (one may have been missed).
                    if (!frame.keyFrame) {
                        socket.send(StreamFrame.KeyFrameRequest.encode().toByteString())
                        continue
                    }
                    keyFrameSeen = true
                }
                val index = generateSequence { codec.dequeueInputBuffer(DEQUEUE_US) }
                    .first { it >= 0 || !open.get() }
                if (index < 0) break
                val captureUs = frame.captureUs.toLong()
                codec.getInputBuffer(index)!!.apply {
                    clear()
                    put(frame.data, frame.offset, frame.length)
                }
                queuedAt[captureUs] = System.nanoTime()
                codec.queueInputBuffer(index, 0, frame.length, captureUs, 0)
            }
        } catch (_: InterruptedException) {
            // Closed.
        } catch (e: IllegalStateException) {
            // MediaCodec.CodecException included.
            log.log(Level.WARNING, "second screen decoder failed", e)
            end(ScreenStopReason.DecoderFailed)
        } finally {
            shower.interrupt()
            shower.join(JOIN_MS)
            runCatching { codec.stop() }
            codec.release()
        }
    }

    private fun show(codec: MediaCodec) {
        val info = MediaCodec.BufferInfo()
        try {
            while (open.get() && !Thread.currentThread().isInterrupted) {
                val index = codec.dequeueOutputBuffer(info, DEQUEUE_US)
                if (index < 0) continue
                codec.releaseOutputBuffer(index, true)
                val captureUs = info.presentationTimeUs
                val decodeUs = queuedAt.remove(captureUs)?.let { (System.nanoTime() - it) / 1000 } ?: 0
                val ack = StreamFrame.Ack(captureUs.toULong(), decodeUs.coerceIn(0, UInt.MAX_VALUE.toLong()).toUInt())
                socket.send(ack.encode().toByteString())
            }
        } catch (e: IllegalStateException) {
            // Stopped while dequeuing (close), or the decoder failed.
            if (open.get()) {
                log.log(Level.WARNING, "second screen decoder failed", e)
                end(ScreenStopReason.DecoderFailed)
            }
        }
    }

    private companion object {
        val log: Logger = Logger.getLogger("ScreenPlayer")
        const val NORMAL_CLOSURE = 1000
        const val POLL_MS = 100L
        const val DEQUEUE_US = 10_000L
        const val JOIN_MS = 1000L
    }
}
