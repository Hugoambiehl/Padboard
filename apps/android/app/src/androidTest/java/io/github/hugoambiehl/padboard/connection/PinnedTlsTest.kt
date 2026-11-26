package io.github.hugoambiehl.padboard.connection

import android.os.Build
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.hugoambiehl.padboard.protocol.Fingerprint
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit
import okhttp3.Handshake
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The Wi-Fi link's TLS on this device's Android (written for API 24, which
 * has no TLS 1.3): a real handshake with a running Padboard desktop, pinned
 * like the app does ([SpkiPinTrustManager]).
 *
 * Needs the PC: `-e padboardHost 10.0.2.2` (the host, from an emulator),
 * optionally `-e padboardPort 47322` and `-e padboardFingerprint XXXX-XXXX-XXXX`
 * (the short fingerprint the PC shows). Skipped without a host, so the
 * other androidTests still run alone (`scripts/tablet-ui-tests.sh`).
 */
@RunWith(AndroidJUnit4::class)
class PinnedTlsTest {
    private val args = InstrumentationRegistry.getArguments()
    private val host: String? = args.getString("padboardHost")
    private val port: Int = args.getString("padboardPort")?.toInt() ?: 47322

    @Before
    fun needsAPc() = assumeTrue("no PC given (-e padboardHost)", host != null)

    @Test
    fun handshakeIsPinnedToThePcsKey() {
        // First contact (pairing): any key, remembered.
        val first = SpkiPinTrustManager(null)
        val handshake = open(first).getOrThrow()
        Log.i(TAG, "API ${Build.VERSION.SDK_INT}: ${handshake.tlsVersion} ${handshake.cipherSuite}")
        val spki = first.acceptedSpki
        assertNotNull("the PC's key was seen", spki)
        args.getString("padboardFingerprint")?.let {
            assertEquals("the PC's fingerprint", it, Fingerprint.short(spki!!))
        }

        // Paired: the same key passes, another one is refused.
        val pin = Fingerprint.spkiSha256(spki!!)
        open(SpkiPinTrustManager(pin)).getOrThrow()
        val wrong = pin.copyOf().also { it[0] = (it[0] + 1).toByte() }
        val refused = open(SpkiPinTrustManager(wrong)).exceptionOrNull()
        Log.i(TAG, "wrong pin: $refused")
        assertTrue("a wrong pin must fail the handshake, got $refused", refused != null)
    }

    /** Opens `wss://host:port` (the TLS handshake and the WebSocket upgrade), then closes it. */
    private fun open(trust: SpkiPinTrustManager): Result<Handshake> {
        val client = OkHttpClient.Builder().connectTimeout(10, TimeUnit.SECONDS).build().pinnedTo(trust)
        val result = CompletableFuture<Result<Handshake>>()
        client.newWebSocket(
            Request.Builder().url("wss://$host:$port").build(),
            object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) {
                    webSocket.close(1000, null)
                    result.complete(
                        response.handshake?.let { Result.success(it) }
                            ?: Result.failure(IllegalStateException("no TLS handshake"))
                    )
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    result.complete(Result.failure(t))
                }
            }
        )
        return try {
            result.get(20, TimeUnit.SECONDS)
        } finally {
            client.dispatcher.executorService.shutdown()
            client.connectionPool.evictAll()
        }
    }

    private companion object {
        const val TAG = "PinnedTlsTest"
    }
}
