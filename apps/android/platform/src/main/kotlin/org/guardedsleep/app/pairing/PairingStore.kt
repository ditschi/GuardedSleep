package org.guardedsleep.app.pairing

import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import uniffi.guardedsleep_core.InviteMailbox
import uniffi.guardedsleep_core.MailboxKind

data class TurnCredentials(val url: String, val username: String, val credential: String) {
    // The password must never reach logs through string templates.
    override fun toString(): String = "TurnCredentials(url=<redacted>, credential=<redacted>)"
}

/** One mailbox of the pairing's set: `kind` is `relay` or `lan` (the core's wire names). */
data class StoredMailbox(val kind: String, val url: String) {
    /** The core's entry. A kind this build does not know is no address. */
    fun toInviteMailbox(): InviteMailbox? =
        when (kind) {
            "relay" -> MailboxKind.RELAY
            "lan" -> MailboxKind.LAN
            else -> null
        }?.let { InviteMailbox(it, url) }
}

/**
 * The set a URL list stands for (`wss://` a relay, `ws://` a LAN mailbox), as an invite carries it.
 */
fun storedMailboxesFor(urls: List<String>): List<StoredMailbox> =
    PairingPayload.mailboxesFor(urls).map { it.toStored() }

fun InviteMailbox.toStored(): StoredMailbox =
    StoredMailbox(
        when (kind) {
            MailboxKind.RELAY -> "relay"
            MailboxKind.LAN -> "lan"
        },
        url,
    )

/**
 * The signaling URLs of a stored set: the relay and LAN entries only. An earlier alpha record may
 * still name a removed kind, which is no address to open.
 */
internal fun List<StoredMailbox>.signalingUrls(): List<String> = filter {
    it.kind == "relay" || it.kind == "lan"
}
    .map { it.url }

/** The one saved pairing of this phone (ADR-018: 1 baby + 1 parent). */
data class StoredPairing(
    val pskB64url: String,
    val signalingUrls: List<String>,
    /** The PSK verified at least once, so a later mismatch is never terminal (ADR-017 §3a). */
    val pairedBefore: Boolean,
    val turn: TurnCredentials? = null,
    /**
     * The pinned parent secret (base64url of 32 bytes): the baby's pinned parent, or the secret the
     * parent drew (acknowledged or not). Null before the first pairing completed. The record holds
     * it durably before the parent sends it to the baby (persist before send).
     */
    val parentSecretB64url: String? = null,
    /**
     * The mailbox set, the parent's copy of the one the baby sent after a verification (empty: none
     * was received yet, [signalingUrls] is all there is).
     */
    val mailboxes: List<StoredMailbox> = emptyList(),
    /**
     * Parent: the end of the first-pairing window (Unix seconds, the invite expiry) while the baby
     * has not acknowledged the secret; the raw secret is never sent after it. Null once
     * acknowledged (and on the baby).
     */
    val pinUntilS: Long? = null,
) {
    // The PSK and the parent secret must never reach logs through string templates.
    override fun toString(): String =
        "StoredPairing(pairedBefore=$pairedBefore, urls=${signalingUrls.size}, " +
            "pinned=${parentSecretB64url != null}, mailboxes=${mailboxes.size})"
}

/** The outcome of [PairingStore.loadResult]. */
sealed interface PairingLoad {
    /** No pairing (never saved, or unrecoverable and deleted). */
    data object Absent : PairingLoad

    /** The record is kept but cannot be read right now (Keystore busy, device still locked). */
    data object Unreadable : PairingLoad

    class Loaded(val pairing: StoredPairing) : PairingLoad
}

/**
 * PSK, server list and TURN credentials sealed with [PairingCipher] in an app-private file. Plain
 * preferences never hold pairing material (guardrail 7.2).
 */
class PairingStore(file: File, cipher: PairingCipher) {
    private val sealed = SealedJsonFile(file, cipher)

    /** The pairing, or null when there is none or it cannot be read right now (never cleared). */
    @Synchronized fun load(): StoredPairing? = (loadResult() as? PairingLoad.Loaded)?.pairing

    /**
     * Like [load] but tells "none" from "temporarily unreadable": an unrecoverable record is
     * deleted ([PairingLoad.Absent]); a transient failure keeps it ([PairingLoad.Unreadable]).
     */
    @Synchronized
    fun loadResult(): PairingLoad {
        val o =
            when (val r = sealed.read()) {
                SealedRead.Absent,
                SealedRead.Cleared -> return PairingLoad.Absent
                is SealedRead.Unreadable -> return PairingLoad.Unreadable
                is SealedRead.Value -> r.json
            }
        // A well-formed file with missing fields is treated like an unreadable one.
        return runCatching<PairingLoad> {
                val urls =
                    o.getJSONArray("s").let { a -> (0 until a.length()).map { a.getString(it) } }
                val turn = o.optJSONObject("t")?.let(::turnCredentialsFromJson)
                val mailboxes =
                    o.optJSONArray("m")?.let { a ->
                        (0 until a.length()).map {
                            val m = a.getJSONObject(it)
                            StoredMailbox(m.getString("k"), m.getString("u"))
                        }
                    } ?: emptyList()
                PairingLoad.Loaded(
                    StoredPairing(
                        o.getString("k"),
                        urls,
                        o.optBoolean("p", false),
                        turn,
                        o.optString("ps").ifEmpty { null },
                        mailboxes,
                        o.optLong("pu", -1L).takeIf { it >= 0 },
                    )
                )
            }
            .getOrElse {
                sealed.clear()
                PairingLoad.Absent
            }
    }

    @Synchronized
    fun save(pairing: StoredPairing) {
        val o =
            JSONObject()
                .put("k", pairing.pskB64url)
                .put("s", JSONArray(pairing.signalingUrls))
                .put("p", pairing.pairedBefore)
        pairing.turn?.let { o.put("t", it.toJson()) }
        pairing.parentSecretB64url?.let { o.put("ps", it) }
        pairing.pinUntilS?.let { o.put("pu", it) }
        if (pairing.mailboxes.isNotEmpty()) {
            o.put(
                "m",
                JSONArray(
                    pairing.mailboxes.map { JSONObject().put("k", it.kind).put("u", it.url) }
                ),
            )
        }
        // A caller that sends the parent secret to the baby after `save` returned must find it
        // again after a crash; `write` syncs before the rename and throws on failure.
        sealed.write(o)
    }

    @Synchronized
    fun markPairedBefore() {
        load()?.takeIf { !it.pairedBefore }?.let { save(it.copy(pairedBefore = true)) }
    }

    /**
     * Stores the parent secret and the first-pairing window in the same encrypted record. False
     * when there is no pairing yet (the caller keeps the secret until the record exists). Throws
     * when the record cannot be written.
     */
    @Synchronized
    fun setParentSecret(secretB64url: String, pinUntilS: Long? = null): Boolean {
        val stored = load() ?: return false
        save(stored.copy(parentSecretB64url = secretB64url, pinUntilS = pinUntilS))
        return true
    }

    /**
     * Replaces the stored mailbox set (the baby's update). The URL list derived from it is kept for
     * the record format only; the platform opens the set, never the list. False when there is no
     * pairing yet.
     */
    @Synchronized
    fun setMailboxes(mailboxes: List<StoredMailbox>): Boolean {
        val stored = load() ?: return false
        val urls = mailboxes.signalingUrls()
        save(
            stored.copy(
                mailboxes = mailboxes,
                signalingUrls = urls.ifEmpty { stored.signalingUrls },
            )
        )
        return true
    }

    @Synchronized fun exists(): Boolean = sealed.exists()

    /** Deletes the file and the key alias. */
    @Synchronized fun clear() = sealed.clear()
}
