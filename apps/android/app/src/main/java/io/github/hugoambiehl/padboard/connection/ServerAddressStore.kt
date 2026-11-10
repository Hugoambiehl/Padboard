package io.github.hugoambiehl.padboard.connection

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.connectionDataStore: DataStore<Preferences> by preferencesDataStore(name = "connection")

/** Remembers the last address the tablet connected to, to reconnect at startup. */
class ServerAddressStore(context: Context) {
    private val dataStore = context.applicationContext.connectionDataStore

    /** Last saved address, or `null` if the tablet never connected. */
    val address: Flow<ServerAddress?> =
        dataStore.data.map { prefs ->
            val host = prefs[HOST] ?: return@map null
            ServerAddress(host, prefs[PORT] ?: ServerAddress.DEFAULT_PORT)
        }

    suspend fun save(address: ServerAddress) {
        dataStore.edit { prefs ->
            prefs[HOST] = address.host
            prefs[PORT] = address.port
        }
    }

    private companion object {
        val HOST = stringPreferencesKey("host")
        val PORT = intPreferencesKey("port")
    }
}
