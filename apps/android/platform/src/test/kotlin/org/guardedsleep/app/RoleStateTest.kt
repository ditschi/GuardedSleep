package org.guardedsleep.app

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.guardedsleep.app.fixtures.FakeCipher
import org.guardedsleep.app.pairing.PairingCipher
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Start mode, per-role values and per-role pairing slots, including the update from one role. */
class RoleStateTest {
    @get:Rule val tmp = TemporaryFolder()

    private val scope = CoroutineScope(Dispatchers.Unconfined)

    private fun dataStore(name: String = "s") =
        PreferenceDataStoreFactory.create(scope = scope) { File(tmp.root, "$name.preferences_pb") }

    private fun androidx.datastore.core.DataStore<Preferences>.stored(): Preferences = runBlocking {
        data.first()
    }

    // ---- SettingsRepository ------------------------------------------------------------------

    @Test
    fun aNewInstallHasNoRoleAndNoDefault() {
        val repo = SettingsRepository(dataStore(), scope)
        assertNull(repo.current.role)
        assertNull(repo.current.defaultRole)
    }

    @Test
    fun theStoredRoleBecomesTheDefaultOnTheFirstRead() {
        val ds = dataStore()
        runBlocking {
            ds.edit {
                it[stringPreferencesKey("role")] = "BABY"
                it[stringPreferencesKey("peer_name")] = "Old parent"
                it[longPreferencesKey("snooze_until_wall")] = 7L
            }
        }
        val repo = SettingsRepository(ds, scope)
        assertEquals(Role.BABY, repo.current.role)
        assertEquals(Role.BABY, repo.current.defaultRole)
        // The unsuffixed values are that role's.
        assertEquals("Old parent", repo.current.peerName)
        assertEquals(7L, repo.current.snoozeUntilWallMs)

        repo.update { it.copy(onboardingDone = true) }
        runBlocking { ds.edit {} }
        val p = ds.stored()
        assertEquals("BABY", p[stringPreferencesKey("default_role")])
        assertEquals("Old parent", p[stringPreferencesKey("peer_name_baby")])
        assertNull(p[stringPreferencesKey("peer_name")])
        // A second repository on the migrated data reads the same state.
        val again = SettingsRepository(ds, scope)
        assertEquals(Role.BABY, again.current.defaultRole)
        assertEquals("Old parent", again.current.peerName)
    }

    @Test
    fun askEveryTimeIsStoredAndNotMigratedAgain() {
        val ds = dataStore()
        val repo = SettingsRepository(ds, scope)
        repo.update { it.copy(role = Role.PARENT, defaultRole = null) }
        runBlocking { ds.edit {} }
        val again = SettingsRepository(ds, scope)
        assertEquals(Role.PARENT, again.current.role)
        assertNull("an explicit Ask every time stays", again.current.defaultRole)
    }

    @Test
    fun aRoleChangeMovesTheValuesOfTheOldRoleAside() {
        val ds = dataStore()
        val repo = SettingsRepository(ds, scope)
        repo.update { it.copy(role = Role.PARENT, defaultRole = Role.PARENT) }
        repo.update { it.copy(peerName = "Lena", snoozeUntilWallMs = 5L, lastTestWallMs = 9L) }
        repo.update { it.copy(role = Role.BABY) }
        assertNull(repo.current.peerName)
        assertNull(repo.current.snoozeUntilWallMs)
        assertNull(repo.current.lastTestWallMs)
        repo.update { it.copy(peerName = "Parent phone") }
        runBlocking { ds.edit {} }

        val restarted = SettingsRepository(ds, scope)
        assertEquals(Role.BABY, restarted.current.role)
        assertEquals("Parent phone", restarted.current.peerName)
        restarted.update { it.copy(role = Role.PARENT) }
        assertEquals("Lena", restarted.current.peerName)
        assertEquals(5L, restarted.current.snoozeUntilWallMs)
        assertEquals(9L, restarted.current.lastTestWallMs)
        assertEquals(Role.PARENT, restarted.current.defaultRole)
    }

    // ---- RolePairings ------------------------------------------------------------------------

    private class Slots(dir: File) {
        val ciphers = Role.entries.associateWith { FakeCipher() }
        val legacyCipher = FakeCipher()
        val legacy = PairingStore(File(dir, "pairing.bin"), legacyCipher)
        val pairings =
            RolePairings(
                Role.entries.associateWith {
                    listOf(
                        PairingStore(
                            File(dir, "pairing-${it.name.lowercase()}.bin"),
                            ciphers.getValue(it),
                        ),
                        PairingStore(
                            File(dir, "pairing-${it.name.lowercase()}-2.bin"),
                            FakeCipher(),
                        ),
                    )
                },
                legacy,
            )
    }

    private fun pairing(psk: String) = StoredPairing(psk, listOf("wss://h.example"), true)

    @Test
    fun theSlotsAreIndependent() {
        val s = Slots(tmp.root)
        s.pairings[Role.BABY].save(pairing("baby"))
        s.pairings[Role.PARENT].save(pairing("parent"))
        s.pairings[Role.PARENT].clear()
        assertEquals("baby", s.pairings[Role.BABY].load()?.pskB64url)
        assertNull(s.pairings[Role.PARENT].load())
        assertTrue(s.ciphers.getValue(Role.PARENT).keyDeleted)
        assertFalse("the other role keeps its key", s.ciphers.getValue(Role.BABY).keyDeleted)
    }

    @Test
    fun theLegacyFileMovesToTheSlotOfTheRole() {
        val s = Slots(tmp.root)
        s.legacy.save(pairing("legacy"))
        s.pairings.migrateLegacy(Role.PARENT)
        assertEquals("legacy", s.pairings[Role.PARENT].load()?.pskB64url)
        assertNull(s.pairings[Role.BABY].load())
        assertFalse(s.legacy.exists())
        assertTrue(s.legacyCipher.keyDeleted)
        // Idempotent.
        s.pairings.migrateLegacy(Role.PARENT)
        assertEquals("legacy", s.pairings[Role.PARENT].load()?.pskB64url)
    }

    @Test
    fun noRoleLeavesTheLegacyFileAlone() {
        val s = Slots(tmp.root)
        s.legacy.save(pairing("legacy"))
        s.pairings.migrateLegacy(null)
        assertTrue(s.legacy.exists())
    }

    @Test
    fun anUnreadableLegacyFileIsKeptAndRetried() {
        val flaky = FlakyCipher(1) { IllegalStateException("keystore busy") }
        val legacyFile = File(tmp.root, "pairing.bin")
        PairingStore(legacyFile, FakeCipher()).save(pairing("legacy"))
        val slots =
            Role.entries.associateWith {
                listOf(
                    PairingStore(
                        File(tmp.root, "pairing-${it.name.lowercase()}.bin"),
                        FakeCipher(),
                    ),
                    PairingStore(
                        File(tmp.root, "pairing-${it.name.lowercase()}-2.bin"),
                        FakeCipher(),
                    ),
                )
            }
        val pairings = RolePairings(slots, PairingStore(legacyFile, flaky))

        pairings.migrateLegacy(Role.BABY)
        assertTrue("kept for the next start", legacyFile.exists())
        assertNull(pairings[Role.BABY].load())

        pairings.migrateLegacy(Role.BABY)
        assertEquals("legacy", pairings[Role.BABY].load()?.pskB64url)
        assertFalse(legacyFile.exists())
    }

    @Test
    fun aFailedSlotWriteKeepsTheLegacyFile() {
        val legacyFile = File(tmp.root, "pairing.bin")
        val legacy = PairingStore(legacyFile, FakeCipher())
        legacy.save(pairing("legacy"))
        val failing =
            object : PairingCipher {
                override fun encrypt(plain: ByteArray): ByteArray = error("disk full")

                override fun decrypt(sealed: ByteArray): ByteArray = sealed

                override fun deleteKey() = Unit
            }
        val slots =
            Role.entries.associateWith {
                listOf(
                    PairingStore(File(tmp.root, "pairing-${it.name.lowercase()}.bin"), failing),
                    PairingStore(File(tmp.root, "pairing-${it.name.lowercase()}-2.bin"), failing),
                )
            }
        RolePairings(slots, legacy).migrateLegacy(Role.BABY)
        assertTrue(legacyFile.exists())
        assertNull(slots.getValue(Role.BABY).first().load())
    }
}
