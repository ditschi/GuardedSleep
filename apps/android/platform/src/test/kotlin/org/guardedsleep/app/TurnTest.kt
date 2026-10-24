package org.guardedsleep.app

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnServerInput
import org.guardedsleep.app.pairing.TurnServerInput.Result
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.settings.SettingsRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class TurnTest {
    @get:Rule val tmp = TemporaryFolder()

    private fun address(url: String) = TurnServerInput.validate(url, "", "")

    @Test
    fun validAddresses() {
        listOf(
                "turn:203.0.113.10:3478",
                "turns:turn.example.org:5349?transport=tcp",
                "turn:[2001:db8::1]:3478",
                "turn:host",
                "  turn:host:65535?transport=udp  ",
            )
            .forEach { assertTrue(it, address(it) is Result.Ok) }
    }

    @Test
    fun invalidAddresses() {
        listOf(
                "stun:x",
                "turn:",
                "turn: host",
                "turn:host:0",
                "turn:host:70000",
                "turn:host?transport=sctp",
                "https://x",
                "turn:-host",
                "turn:.host",
                "",
            )
            .forEach { assertEquals(it, Result.InvalidAddress, address(it)) }
    }

    @Test
    fun addressAndUserNameAreTrimmedPasswordIsNot() {
        val r = TurnServerInput.validate(" turn:h ", " u ", " pw ")
        assertEquals(Result.Ok("turn:h", "u", " pw "), r)
    }

    @Test
    fun credentialRules() {
        val url = "turn:h"
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, "u", ""))
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, "", "pw"))
        val long = "a".repeat(257)
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, long, "pw"))
        assertEquals(Result.InvalidCredentials, TurnServerInput.validate(url, "u", long))
        val max = "a".repeat(256)
        assertTrue(TurnServerInput.validate(url, max, max) is Result.Ok)
        assertTrue(TurnServerInput.validate(url, "", "") is Result.Ok)
    }

    private val saved = TurnCredentials("turn:h", "u", "pw")

    private fun savedEntry(r: TurnServerInput.Resolution) =
        (r as TurnServerInput.Resolution.Saved).turn

    @Test
    fun resolveKeepsTheSavedPasswordForAnUnchangedEntry() {
        assertEquals(saved, savedEntry(TurnServerInput.resolve(saved, "turn:h", "u", "")))
    }

    @Test
    fun resolveDoesNotKeepItWhenAddressOrUserNameChanged() {
        val invalid = TurnServerInput.Resolution.Invalid(Result.InvalidCredentials)
        assertEquals(invalid, TurnServerInput.resolve(saved, "turn:other", "u", ""))
        assertEquals(invalid, TurnServerInput.resolve(saved, "turn:h", "v", ""))
    }

    @Test
    fun resolveTrimsAddressAndUserNameButNotThePassword() {
        assertEquals(
            TurnCredentials("turn:h", "u", " pw "),
            savedEntry(TurnServerInput.resolve(null, " turn:h ", " u ", " pw ")),
        )
    }

    @Test
    fun resolveClearsOnAnEmptyAddress() {
        assertEquals(
            TurnServerInput.Resolution.Cleared,
            TurnServerInput.resolve(saved, " ", "", ""),
        )
    }

    @Test
    fun resolveReportsAnInvalidAddress() {
        assertEquals(
            TurnServerInput.Resolution.Invalid(Result.InvalidAddress),
            TurnServerInput.resolve(saved, "stun:x", "u", "pw"),
        )
    }

    @Test
    fun storeRoundTripAndClear() {
        val cipher = FakeCipher()
        val file = File(tmp.root, "turn.bin")
        val store = TurnStore(file, cipher)
        assertNull(store.load())
        store.save(TurnCredentials("turn:h", "u", "pw"))
        val loaded = store.load()
        assertEquals("pw", loaded?.credential)
        assertFalse(String(file.readBytes(), Charsets.UTF_8).contains("pw"))
        store.clear()
        assertNull(store.load())
        assertFalse(file.exists())
        assertTrue(cipher.keyDeleted)
    }

    @Test
    fun unreadableFileIsClearedAndNull() {
        val file = File(tmp.root, "turn.bin")
        file.writeBytes(byteArrayOf(1, 2, 3))
        assertNull(TurnStore(file, FakeCipher()).load())
        assertFalse(file.exists())
    }

    @Test
    fun credentialsToStringHidesPassword() {
        assertFalse(TurnCredentials("turn:h", "u", "secretpw").toString().contains("secretpw"))
    }

    @Test
    fun legacyTurnUrlMovesIntoTheSealedStoreOnce() {
        val scope = CoroutineScope(Dispatchers.Unconfined)
        val prefs =
            PreferenceDataStoreFactory.create(scope = scope) {
                File(tmp.root, "legacy.preferences_pb")
            }
        runBlocking { prefs.edit { it[stringPreferencesKey("turn_url")] = "turn:old:3478" } }
        val turn = TurnStore(File(tmp.root, "turn.bin"), FakeCipher())
        val repo = SettingsRepository(prefs, scope, turn)
        assertEquals(TurnCredentials("turn:old:3478", "", "").url, turn.load()?.url)
        assertEquals("", turn.load()?.username)
        assertNull(repo.current.settings.turnUrl)
        assertNull(runBlocking { prefs.data.first()[stringPreferencesKey("turn_url")] })

        // A second start must not overwrite an entry the user saved meanwhile.
        turn.save(TurnCredentials("turn:new", "u", "pw"))
        SettingsRepository(prefs, scope, turn)
        assertEquals("turn:new", turn.load()?.url)
    }
}
