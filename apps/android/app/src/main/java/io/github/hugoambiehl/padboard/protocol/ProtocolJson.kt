package io.github.hugoambiehl.padboard.protocol

import kotlinx.serialization.json.Json

/** Version of the wire protocol, checked by the desktop during the handshake. Mirrors `PROTOCOL_VERSION`. */
const val PROTOCOL_VERSION: UInt = 1u

/**
 * JSON configuration matching the Rust side (`crates/protocol`):
 * - `{ "type": "...", "payload": {...} }` envelope: the sealed [Message] uses `type` as discriminator
 *   and each variant has a single `payload` property;
 * - unknown keys are ignored (adding optional fields is backward compatible);
 * - absent optional values are omitted, like `skip_serializing_if = "Option::is_none"`.
 */
val ProtocolJson: Json =
    Json {
        classDiscriminator = "type"
        ignoreUnknownKeys = true
        explicitNulls = false
    }

fun Message.toJson(): String = ProtocolJson.encodeToString(Message.serializer(), this)

/** @throws kotlinx.serialization.SerializationException if [text] is not a known message. */
fun parseMessage(text: String): Message = ProtocolJson.decodeFromString(Message.serializer(), text)
