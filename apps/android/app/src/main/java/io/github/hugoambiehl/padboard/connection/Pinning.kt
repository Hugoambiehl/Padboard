package io.github.hugoambiehl.padboard.connection

import android.annotation.SuppressLint
import io.github.hugoambiehl.padboard.protocol.Fingerprint
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient

/**
 * Wi-Fi TLS: the PC's certificate is self-signed, so instead of a CA the
 * tablet pins the SHA-256 of the PC's public key (SPKI) received at pairing,
 * like `apps/desktop/src-tauri/src/pinning.rs`. Names and dates are ignored:
 * only the key matters, and the TLS handshake proves the PC holds it.
 *
 * `pin == null` is the first contact (pairing): any key is accepted and the
 * user compares the short fingerprint shown on both screens.
 */
@SuppressLint("CustomX509TrustManager")
class SpkiPinTrustManager(private val pin: ByteArray?) : X509TrustManager {
    /** The key of the last accepted certificate (shown as a fingerprint when pairing). */
    @Volatile var acceptedSpki: ByteArray? = null
        private set

    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        val spki = chain?.firstOrNull()?.publicKey?.encoded ?: throw CertificateException("no certificate")
        if (pin != null && !MessageDigest.isEqual(Fingerprint.spkiSha256(spki), pin)) {
            throw CertificateException("the PC's key does not match the pinned one")
        }
        acceptedSpki = spki
    }

    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?): Unit =
        throw CertificateException("no client certificates")

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
}

/** A client for `wss://` to the PC, enforcing [pin] (see [SpkiPinTrustManager]). */
fun OkHttpClient.pinnedTo(pin: ByteArray?): OkHttpClient = pinnedTo(SpkiPinTrustManager(pin))

/** A client for `wss://` to the PC, checked by [trustManager]. */
fun OkHttpClient.pinnedTo(trustManager: SpkiPinTrustManager): OkHttpClient {
    val context = SSLContext.getInstance("TLS").apply { init(null, arrayOf(trustManager), null) }
    return newBuilder()
        .sslSocketFactory(context.socketFactory, trustManager)
        // The PC is reached by IP and its certificate names nothing: the pinned
        // key replaces hostname verification.
        .hostnameVerifier { _, _ -> true }
        .build()
}
