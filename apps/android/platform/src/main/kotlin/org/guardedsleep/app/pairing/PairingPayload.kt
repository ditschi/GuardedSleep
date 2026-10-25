package org.guardedsleep.app.pairing

import java.util.Base64
import org.guardedsleep.app.config.PairingPolicy
import org.guardedsleep.app.platform.BuildConfig
import org.guardedsleep.app.ui.model.PairingError
import uniffi.guardedsleep_core.InviteException
import uniffi.guardedsleep_core.InviteMailbox
import uniffi.guardedsleep_core.MailboxKind
import uniffi.guardedsleep_core.buildInvite
import uniffi.guardedsleep_core.parseInvite

/**
 * The v4 invite as the platform holds it. Building and parsing the text is the core's job (one
 * codec for every client, via the binding); this class only carries the values and maps the core's
 * rejection onto [PairingError].
 *
 * Until the core opens mailboxes itself (alpha-connection-model tasks 3.5/3.6) the platform still
 * holds the signaling URL list: a `wss://` URL is shown as an own relay (`r:`), a `ws://` URL as a
 * LAN mailbox (`l:`, which the core accepts only for RFC 1918 and ULA literals), and a received
 * invite's relay and LAN mailboxes are opened as signaling URLs; Nostr mailboxes are ignored.
 */
data class PairingPayload(
    val pskB64url: String,
    val signalingUrls: List<String>,
    val expiresAtEpochSec: Long,
) {
    /**
     * The invite text, or null when the core refuses the URL list (for example a `ws://` address
     * that is not a private LAN address). Contains the PSK: never log, never put into an Intent or
     * Bundle.
     */
    fun toInviteText(): String? =
        try {
            buildInvite(
                decodePsk(pskB64url) ?: ByteArray(0),
                mailboxesFor(signalingUrls),
                expiresAtEpochSec.toULong(),
                ALLOW_DEV_LOOPBACK,
            )
        } catch (e: InviteException) {
            null
        }

    companion object {
        /**
         * Debug builds only: the core also accepts loopback `l:` mailboxes, for a signaling server
         * on the developer's machine. A release build never does.
         */
        private val ALLOW_DEV_LOOPBACK = BuildConfig.DEBUG

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

        private fun mailboxesFor(urls: List<String>): List<InviteMailbox> = urls.mapNotNull {
            when {
                it.startsWith("wss://") -> InviteMailbox(MailboxKind.RELAY, it)
                it.startsWith("ws://") -> InviteMailbox(MailboxKind.LAN, it)
                else -> null
            }
        }

        fun b64(bytes: ByteArray): String =
            Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

        fun decodePsk(b64url: String): ByteArray? = runCatching {
            Base64.getUrlDecoder().decode(b64url)
        }
            .getOrNull()

        /** Accepts the decoded QR text or a pasted code (surrounding whitespace is ignored). */
        fun parse(text: String, nowWallMs: Long): Result =
            try {
                val invite =
                    parseInvite(
                        text,
                        (nowWallMs / 1000).coerceAtLeast(0).toULong(),
                        ALLOW_DEV_LOOPBACK,
                    )
                Result.Ok(
                    PairingPayload(
                        b64(invite.psk),
                        invite.mailboxes.filter { it.kind != MailboxKind.NOSTR }.map { it.url },
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
