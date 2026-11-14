package io.github.hugoambiehl.padboard.connection

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.util.Base64
import kotlinx.coroutines.flow.first

/** The PC this tablet is paired with over Wi-Fi (one at a time). */
data class PairedPc(
    val serverId: String,
    val serverName: String,
    val token: String,
    /** SHA-256 of the PC's public key, base64: what TLS is pinned to. */
    val spkiSha256: String,
    /** Last address that worked, tried when mDNS has not answered yet. */
    val lastEndpoint: Endpoint?
) {
    val pin: ByteArray get() = Base64.getDecoder().decode(spkiSha256)

    /** Without the token, so a log line can never reveal it. */
    override fun toString(): String = "PairedPc(serverId=$serverId, serverName=$serverName, " +
        "token=<redacted>, spkiSha256=$spkiSha256, lastEndpoint=$lastEndpoint)"
}

/** Storage of the [PairedPc]. Abstracted for the tests. */
interface PairingRepository {
    suspend fun load(): PairedPc?

    suspend fun save(pc: PairedPc)

    suspend fun clear()

    suspend fun updateEndpoint(endpoint: Endpoint)
}

private val Context.pairingDataStore: DataStore<Preferences> by preferencesDataStore(name = "pairing")

/** DataStore-backed [PairingRepository]. Excluded from backups (`allowBackup=false`). */
class PairingStore(context: Context) : PairingRepository {
    private val dataStore = context.applicationContext.pairingDataStore

    override suspend fun load(): PairedPc? {
        val prefs = dataStore.data.first()
        val host = prefs[HOST]
        return PairedPc(
            serverId = prefs[SERVER_ID] ?: return null,
            serverName = prefs[SERVER_NAME].orEmpty(),
            token = prefs[TOKEN] ?: return null,
            spkiSha256 = prefs[SPKI] ?: return null,
            lastEndpoint = host?.let { Endpoint(it, prefs[PORT] ?: Endpoint.WIFI_PORT) }
        )
    }

    override suspend fun save(pc: PairedPc) {
        dataStore.edit { prefs ->
            prefs.clear()
            prefs[SERVER_ID] = pc.serverId
            prefs[SERVER_NAME] = pc.serverName
            prefs[TOKEN] = pc.token
            prefs[SPKI] = pc.spkiSha256
            pc.lastEndpoint?.let {
                prefs[HOST] = it.host
                prefs[PORT] = it.port
            }
        }
    }

    override suspend fun clear() {
        dataStore.edit { it.clear() }
    }

    override suspend fun updateEndpoint(endpoint: Endpoint) {
        dataStore.edit { prefs ->
            prefs[HOST] = endpoint.host
            prefs[PORT] = endpoint.port
        }
    }

    private companion object {
        val SERVER_ID = stringPreferencesKey("server_id")
        val SERVER_NAME = stringPreferencesKey("server_name")
        val TOKEN = stringPreferencesKey("token")
        val SPKI = stringPreferencesKey("spki_sha256")
        val HOST = stringPreferencesKey("host")
        val PORT = intPreferencesKey("port")
    }
}
