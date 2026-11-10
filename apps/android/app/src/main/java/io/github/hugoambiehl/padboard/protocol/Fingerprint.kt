package io.github.hugoambiehl.padboard.protocol

import java.security.MessageDigest

/**
 * Short, human-comparable fingerprint of the PC's TLS public key, identical to
 * `crates/protocol/src/fingerprint.rs` (same test vector in
 * `crates/protocol/fixtures/vectors/fingerprint.json`).
 *
 * Input: the DER-encoded SubjectPublicKeyInfo (`X509Certificate.publicKey.encoded`).
 * Code: the first 60 bits of its SHA-256 in Crockford base32, grouped by 4
 * (`XXXX-XXXX-XXXX`).
 */
object Fingerprint {
    private const val ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
    private const val CHARS = 12
    private const val GROUP = 4

    /** What the tablet pins. */
    fun spkiSha256(spki: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(spki)

    fun short(spki: ByteArray): String {
        val hash = spkiSha256(spki)
        var bits = 0L
        for (i in 0 until 8) bits = (bits shl 8) or (hash[i].toLong() and 0xff)
        return buildString {
            for (i in 0 until CHARS) {
                if (i > 0 && i % GROUP == 0) append('-')
                val index = ((bits ushr (64 - 5 * (i + 1))) and 0x1f).toInt()
                append(ALPHABET[index])
            }
        }
    }
}
