package org.guardedsleep.app.pairing

import java.io.File
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.Role
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

/** One saved pairing (1 baby + 1 parent); a phone keeps up to [RolePairings.MAX_PER_ROLE]. */
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
    /**
     * The local connection name the user chose (or the peer's device name): null until there is
     * one. Sealed with the record, never sent to a peer or relay, never logged.
     */
    val connectionName: String? = null,
) {
    // The PSK, the parent secret and the connection name must never reach logs through string
    // templates.
    override fun toString(): String =
        "StoredPairing(pairedBefore=$pairedBefore, urls=${signalingUrls.size}, " +
            "pinned=${parentSecretB64url != null}, mailboxes=${mailboxes.size})"
}

/** The rules of a local connection name: free text of 1 to 30 characters. */
object ConnectionName {
    const val MAX_LENGTH = ConnectionUi.MAX_NAME_LENGTH

    /** The trimmed name, or null when it is empty or longer than [MAX_LENGTH] characters. */
    fun normalize(text: String): String? {
        val t = text.trim()
        return t.takeIf { it.isNotEmpty() && it.codePointCount(0, it.length) <= MAX_LENGTH }
    }

    /** A peer's device name as a default name: trimmed and cut to [MAX_LENGTH], null if blank. */
    fun fromPeerDevice(text: String): String? {
        val t = text.trim()
        if (t.isEmpty()) return null
        val end =
            if (t.codePointCount(0, t.length) <= MAX_LENGTH) t.length
            else t.offsetByCodePoints(0, MAX_LENGTH)
        return t.substring(0, end).trim().ifEmpty { null }
    }
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
                        o.optString("n").ifEmpty { null },
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
        pairing.connectionName?.let { o.put("n", it) }
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

    /**
     * Load, change and save under one hold of the store lock, so a concurrent write of the core
     * thread (parent secret, mailbox set) is never overwritten with stale data. [change] returns
     * the record to save, or null to leave it alone. False when there is no readable record or
     * nothing was saved.
     */
    @Synchronized
    fun update(change: (StoredPairing) -> StoredPairing?): Boolean {
        val stored = load() ?: return false
        save(change(stored) ?: return false)
        return true
    }

    @Synchronized
    fun markPairedBefore() {
        update { if (it.pairedBefore) null else it.copy(pairedBefore = true) }
    }

    /**
     * Stores the parent secret and the first-pairing window in the same encrypted record. False
     * when there is no pairing yet (the caller keeps the secret until the record exists). Throws
     * when the record cannot be written.
     */
    @Synchronized
    fun setParentSecret(secretB64url: String, pinUntilS: Long? = null): Boolean {
        return update { it.copy(parentSecretB64url = secretB64url, pinUntilS = pinUntilS) }
    }

    /**
     * Replaces the stored mailbox set (the baby's update). The URL list derived from it is kept for
     * the record format only; the platform opens the set, never the list. False when there is no
     * pairing yet.
     */
    @Synchronized
    fun setMailboxes(mailboxes: List<StoredMailbox>): Boolean {
        val urls = mailboxes.signalingUrls()
        return update {
            it.copy(mailboxes = mailboxes, signalingUrls = urls.ifEmpty { it.signalingUrls })
        }
    }

    @Synchronized fun exists(): Boolean = sealed.exists()

    /** Deletes the file and the key alias. */
    @Synchronized fun clear() = sealed.clear()
}

/**
 * Which pairing is the default of a role: a local id, no secret, so it is read without the Keystore
 * (a busy Keystore must never change which pairing a start uses).
 */
interface DefaultPairingMarker {
    fun get(role: Role): String?

    fun set(role: Role, id: String?)

    /** Kept in memory only (tests). */
    class InMemory : DefaultPairingMarker {
        private val ids = mutableMapOf<Role, String>()

        override fun get(role: Role): String? = ids[role]

        override fun set(role: Role, id: String?) {
            if (id == null) ids.remove(role) else ids[role] = id
        }
    }

    /** One small plain file per role under [dir]. */
    class Files(private val dir: File) : DefaultPairingMarker {
        private fun file(role: Role) = File(dir, "pairing-default-${role.name.lowercase()}")

        override fun get(role: Role): String? = runCatching {
            file(role).takeIf { it.exists() }?.readText()?.trim()
        }
            .getOrNull()
            ?.ifEmpty { null }

        override fun set(role: Role, id: String?) {
            runCatching { if (id == null) file(role).delete() else file(role).writeText(id) }
        }
    }
}

/** One saved pairing of a role as the lists show it. The name is never part of [toString]. */
class SavedPairing(
    /** The stable local id: the slot the record lives in. */
    val id: String,
    /** The connection name, null while there is none (the UI shows a numbered default). */
    val name: String?,
    val isDefault: Boolean,
) {
    override fun toString(): String = "SavedPairing(id=$id, isDefault=$isDefault)"
}

/**
 * The saved pairings of this phone: up to [MAX_PER_ROLE] per role, each in its own sealed file
 * under its own Keystore key, so forgetting one never touches another. The slot index is the stable
 * local id ("1", "2"). The first slot is the file of the one-pairing layout, so an update keeps it
 * as the first entry. Only the pairings of the session role are used.
 */
class RolePairings(
    private val slots: Map<Role, List<PairingStore>>,
    /** The single-role file of the oldest layout; null when there is none to migrate. */
    private val legacy: PairingStore? = null,
    private val defaults: DefaultPairingMarker = DefaultPairingMarker.InMemory(),
) {
    init {
        require(slots.values.all { it.size == MAX_PER_ROLE }) { "$MAX_PER_ROLE slots per role" }
    }

    /** The store of [role]'s default pairing, or the first slot while there is none. */
    operator fun get(role: Role): PairingStore = store(role, defaultId(role) ?: ids(role).first())

    /** The ids of all slots of [role], in slot order. */
    fun ids(role: Role): List<String> = slotsOf(role).indices.map { (it + 1).toString() }

    fun store(role: Role, id: String): PairingStore {
        val list = slotsOf(role)
        val index = id.toIntOrNull()?.minus(1)
        return requireNotNull(index?.let { list.getOrNull(it) }) { "no pairing slot $id" }
    }

    /** The ids of the slots that hold a pairing (readable or not), in slot order. */
    @Synchronized
    fun savedIds(role: Role): List<String> = ids(role).filter { store(role, it).exists() }

    /** The first free slot, null at the cap. */
    @Synchronized
    fun freeId(role: Role): String? = ids(role).firstOrNull { !store(role, it).exists() }

    /** The pairings of [role] that are saved, with their names (null while unreadable or unset). */
    @Synchronized
    fun list(role: Role): List<SavedPairing> {
        val default = markedDefaultId(role)
        return savedIds(role).map {
            SavedPairing(it, store(role, it).load()?.connectionName, it == default)
        }
    }

    /**
     * The default pairing of [role]: the one the user marked while it is still saved, else the
     * first saved one. Null when there is no pairing. Reads no sealed record.
     */
    @Synchronized
    fun defaultId(role: Role): String? {
        val saved = savedIds(role)
        return defaults.get(role)?.takeIf { it in saved } ?: saved.firstOrNull()
    }

    /**
     * The pairing the user marked as default while it is still saved; no fallback. The parent's
     * automatic start and the "default" flag of the lists use this one, never the implied default.
     */
    @Synchronized
    fun markedDefaultId(role: Role): String? =
        defaults.get(role)?.takeIf { it in savedIds(role) }

    /** Removes the default mark of [role]; the baby keeps falling back to its first pairing. */
    @Synchronized
    fun clearDefault(role: Role) {
        defaults.set(role, null)
    }

    /** Makes [id] the default of [role]; false when nothing is saved there. */
    @Synchronized
    fun setDefault(role: Role, id: String): Boolean {
        if (id !in savedIds(role)) return false
        defaults.set(role, id)
        return true
    }

    /** Renames a connection; false when [name] breaks [ConnectionName] or the pairing is gone. */
    @Synchronized
    fun rename(role: Role, id: String, name: String): Boolean {
        val normalized = ConnectionName.normalize(name) ?: return false
        return store(role, id).update { it.copy(connectionName = normalized) }
    }

    /** Sets the name only while there is none (the peer's device name as the default). */
    @Synchronized
    fun nameIfUnset(role: Role, id: String, name: String) {
        val device = ConnectionName.fromPeerDevice(name) ?: return
        store(role, id).update {
            if (it.connectionName == null) it.copy(connectionName = device) else null
        }
    }

    /** Deletes exactly one pairing with its file and Keystore key. */
    @Synchronized
    fun forget(role: Role, id: String) {
        store(role, id).clear()
        if (defaults.get(role) == id) defaults.set(role, null)
    }

    private fun slotsOf(role: Role): List<PairingStore> =
        checkNotNull(slots[role]) { "no pairing slots for $role" }

    /**
     * Moves the legacy single pairing into the first slot of [role] (the role the install had
     * chosen), once. The pairing is sealed again under the slot's key, then the legacy file and key
     * are deleted. A failure or an unreadable legacy file keeps it, so the next start retries.
     */
    @Synchronized
    fun migrateLegacy(role: Role?) {
        val legacy = legacy ?: return
        if (role == null || !legacy.exists()) return
        val target = store(role, ids(role).first())
        if (target.exists()) {
            // An earlier run copied it and only the cleanup is missing.
            legacy.clear()
            return
        }
        val loaded = legacy.loadResult() as? PairingLoad.Loaded ?: return
        if (runCatching { target.save(loaded.pairing) }.isSuccess) legacy.clear()
    }

    companion object {
        /** The cap of saved pairings per role (`multi-parent-pairings`). */
        const val MAX_PER_ROLE = 2
    }
}
