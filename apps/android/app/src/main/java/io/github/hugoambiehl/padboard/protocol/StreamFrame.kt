package io.github.hugoambiehl.padboard.protocol

/**
 * A binary message of the second screen's stream connection, mirrored by hand
 * from `crates/protocol/src/screen.rs` (`StreamFrame`), checked against the
 * shared vectors (`fixtures/vectors/stream.json`). Big-endian:
 *
 * - video (PC → tablet): `01`, flags (`01` = key frame), capture time µs
 *   (u64), access unit;
 * - ack (tablet → PC): `02`, capture time µs (u64), decode time µs (u32);
 * - key frame request (tablet → PC): `03`.
 */
sealed class StreamFrame {
    /** [data] is a view on the received bytes from [offset], [length] long. */
    class Video(val keyFrame: Boolean, val captureUs: ULong, val data: ByteArray, val offset: Int, val length: Int) :
        StreamFrame()

    data class Ack(val captureUs: ULong, val decodeUs: UInt) : StreamFrame()

    data object KeyFrameRequest : StreamFrame()

    fun encode(): ByteArray = when (this) {
        is Video -> {
            val out = ByteArray(VIDEO_HEADER + length)
            out[0] = VIDEO
            out[1] = if (keyFrame) KEY_FRAME else 0
            putLong(out, 2, captureUs)
            data.copyInto(out, VIDEO_HEADER, offset, offset + length)
            out
        }
        is Ack -> {
            val out = ByteArray(ACK_SIZE)
            out[0] = ACK
            putLong(out, 1, captureUs)
            putInt(out, 9, decodeUs)
            out
        }
        KeyFrameRequest -> byteArrayOf(KEY_FRAME_REQUEST)
    }

    /** Why a message was not read. */
    enum class Error { Empty, UnknownKind, Truncated }

    /** The outcome of [decode]: a frame, or why there is none. */
    sealed class Decoded {
        data class Frame(val frame: StreamFrame) : Decoded()

        data class Refused(val error: Error) : Decoded()
    }

    companion object {
        private const val VIDEO: Byte = 1
        private const val ACK: Byte = 2
        private const val KEY_FRAME_REQUEST: Byte = 3
        private const val KEY_FRAME: Byte = 1
        private const val VIDEO_HEADER = 10
        private const val ACK_SIZE = 13

        /** Reads a message; bytes after an ack's fields are ignored. */
        fun decode(bytes: ByteArray): Decoded {
            if (bytes.isEmpty()) return Decoded.Refused(Error.Empty)
            return when (bytes[0]) {
                VIDEO ->
                    if (bytes.size < VIDEO_HEADER) {
                        Decoded.Refused(Error.Truncated)
                    } else {
                        Decoded.Frame(
                            Video(
                                keyFrame = bytes[1].toInt() and KEY_FRAME.toInt() != 0,
                                captureUs = getLong(bytes, 2),
                                data = bytes,
                                offset = VIDEO_HEADER,
                                length = bytes.size - VIDEO_HEADER
                            )
                        )
                    }
                ACK ->
                    if (bytes.size < ACK_SIZE) {
                        Decoded.Refused(Error.Truncated)
                    } else {
                        Decoded.Frame(Ack(getLong(bytes, 1), getLong(bytes, 9, 4).toUInt()))
                    }
                KEY_FRAME_REQUEST -> Decoded.Frame(KeyFrameRequest)
                else -> Decoded.Refused(Error.UnknownKind)
            }
        }

        private fun putLong(out: ByteArray, at: Int, value: ULong) {
            for (i in 0 until 8) out[at + i] = (value shr (56 - 8 * i)).toByte()
        }

        private fun putInt(out: ByteArray, at: Int, value: UInt) {
            for (i in 0 until 4) out[at + i] = (value shr (24 - 8 * i)).toByte()
        }

        private fun getLong(bytes: ByteArray, at: Int, size: Int = 8): ULong {
            var value = 0uL
            for (i in 0 until size) value = (value shl 8) or (bytes[at + i].toULong() and 0xffuL)
            return value
        }
    }
}
