package org.guardedsleep.app.pairing

import java.util.Base64
import org.guardedsleep.app.config.PairingPolicy
import org.guardedsleep.app.ui.model.PairingError
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject

/** QR payload v3: `{"v":3,"k":<psk b64url>,"s":[signaling urls],"x":<expiry epoch seconds>}`. */
data class PairingPayload(val pskB64url: String, val signalingUrls: List<String>, val expiresAtEpochSec: Long) {
    /** Contains the PSK: never log, never put into an Intent or Bundle. */
    fun toJson(): String =
        JSONObject()
            .put("v", PairingPolicy.PAYLOAD_VERSION)
            .put("k", pskB64url)
            .put("s", JSONArray(signalingUrls))
            .put("x", expiresAtEpochSec)
            .toString()

    companion object {
        sealed interface Result {
            data class Ok(val payload: PairingPayload) : Result

            data class Rejected(val error: PairingError) : Result
        }

        fun create(psk: ByteArray, urls: List<String>, nowWallMs: Long): PairingPayload =
            PairingPayload(
                b64(psk),
                urls,
                (nowWallMs + PairingPolicy.QR_VALIDITY_MS) / 1000,
            )

        fun b64(bytes: ByteArray): String = Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

        fun decodePsk(b64url: String): ByteArray? =
            runCatching { Base64.getUrlDecoder().decode(b64url) }.getOrNull()

        /** Accepts the decoded QR text or a pasted code (surrounding whitespace is ignored). */
        fun parse(text: String, nowWallMs: Long): Result {
            val trimmed = text.trim()
            if (trimmed.isEmpty()) return Result.Rejected(PairingError.INCOMPLETE_PASTE)
            val obj =
                try {
                    JSONObject(trimmed)
                } catch (_: JSONException) {
                    // A JSON-looking text that does not parse was most likely cut off while copying.
                    return Result.Rejected(
                        if (trimmed.startsWith("{")) PairingError.INCOMPLETE_PASTE else PairingError.NOT_OUR_CODE
                    )
                }
            if (obj.optInt("v", -1) != PairingPolicy.PAYLOAD_VERSION) return Result.Rejected(PairingError.NOT_OUR_CODE)
            val key = obj.optString("k", "")
            if (decodePsk(key)?.size != PairingPolicy.PSK_BYTES) return Result.Rejected(PairingError.NOT_OUR_CODE)
            val urls = obj.optJSONArray("s")?.let { arr -> (0 until arr.length()).map { arr.optString(it, "") } }
            if (urls.isNullOrEmpty() || urls.any { !isSignalingUrl(it) }) {
                return Result.Rejected(PairingError.NOT_OUR_CODE)
            }
            val expiry = obj.optLong("x", -1)
            if (expiry <= 0) return Result.Rejected(PairingError.NOT_OUR_CODE)
            if (expiry * 1000 <= nowWallMs) return Result.Rejected(PairingError.EXPIRED)
            return Result.Ok(PairingPayload(key, urls, expiry))
        }

        fun isSignalingUrl(url: String): Boolean {
            val u = url.trim()
            val rest = when {
                u.startsWith("ws://") -> u.removePrefix("ws://")
                u.startsWith("wss://") -> u.removePrefix("wss://")
                else -> return false
            }
            return rest.isNotEmpty() && !rest.startsWith("/") && rest.none { it.isWhitespace() }
        }
    }
}
