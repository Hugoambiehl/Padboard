package io.github.hugoambiehl.padboard.connection

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.util.UUID

private val Context.identityDataStore: DataStore<Preferences> by preferencesDataStore(name = "identity")

/** Stable random identifier of this tablet (`deviceId` in `hello`), created on first use. */
class DeviceIdStore(context: Context) {
    private val dataStore = context.applicationContext.identityDataStore

    suspend fun deviceId(): String {
        var id: String? = null
        dataStore.edit { prefs ->
            id = prefs[DEVICE_ID] ?: UUID.randomUUID().toString().also { prefs[DEVICE_ID] = it }
        }
        return checkNotNull(id)
    }

    private companion object {
        val DEVICE_ID = stringPreferencesKey("device_id")
    }
}
