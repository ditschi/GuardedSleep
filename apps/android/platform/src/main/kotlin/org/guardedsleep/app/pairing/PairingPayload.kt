package org.guardedsleep.app.pairing

import java.util.Base64
import org.guardedsleep.app.config.PairingPolicy
import org.guardedsleep.app.ui.model.PairingError
import uniffi.guardedsleep_core.InviteException
import uniffi.guardedsleep_core.buildInvite
import uniffi.guardedsleep_core.parseInvite

/**
 * The v3 invite as the platform holds it. Building and parsing the text is the core's job (one
 * codec for every client, via the binding); this class only carries the values and maps the core's
 * rejection onto [PairingError].
 */
data class PairingPayload(
    val pskB64url: String,
    val signalingUrls: List<String>,
    val expiresAtEpochSec: Long,
) {
    /** Contains the PSK: never log, never put into an Intent or Bundle. */
    fun toJson(): String =
        // The values come from `create` or a successful parse, so the core accepts them.
        buildInvite(
            decodePsk(pskB64url) ?: ByteArray(0),
            signalingUrls,
            expiresAtEpochSec.toULong(),
        )

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

        fun b64(bytes: ByteArray): String =
            Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

        fun decodePsk(b64url: String): ByteArray? = runCatching {
            Base64.getUrlDecoder().decode(b64url)
        }
            .getOrNull()

        /** Accepts the decoded QR text or a pasted code (surrounding whitespace is ignored). */
        fun parse(text: String, nowWallMs: Long): Result =
            try {
                val invite = parseInvite(text, (nowWallMs / 1000).coerceAtLeast(0).toULong())
                Result.Ok(
                    PairingPayload(
                        b64(invite.psk),
                        invite.urls,
                        invite.expiresAtEpochS.toLong(),
                    )
                )
            } catch (e: InviteException) {
                Result.Rejected(
                    when (e) {
                        is InviteException.NotOurCode -> PairingError.NOT_OUR_CODE
                        is InviteException.IncompletePaste -> PairingError.INCOMPLETE_PASTE
                        is InviteException.Expired -> PairingError.EXPIRED
                    }
                )
            }
    }
}
