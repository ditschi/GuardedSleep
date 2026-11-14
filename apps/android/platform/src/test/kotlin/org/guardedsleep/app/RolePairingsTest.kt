package org.guardedsleep.app

import java.io.File
import org.guardedsleep.app.pairing.ConnectionName
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.StoredPairing
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Up to two saved pairings per role: cap, names, default, forgetting exactly one. */
class RolePairingsTest {
    @get:Rule val tmp = TemporaryFolder()

    private val ciphers = Role.entries.associateWith { listOf(FakeCipher(), FakeCipher()) }
    private val pairings by lazy {
        RolePairings(
            Role.entries.associateWith { role ->
                ciphers.getValue(role).mapIndexed { i, c ->
                    PairingStore(File(tmp.root, "pairing-${role.name.lowercase()}-$i.bin"), c)
                }
            }
        )
    }

    private fun pairing(psk: String, name: String? = null) =
        StoredPairing(psk, listOf("wss://h.example"), true, connectionName = name)

    private fun saveTwo(role: Role = Role.PARENT) {
        pairings.store(role, "1").save(pairing("one"))
        pairings.store(role, "2").save(pairing("two"))
    }

    @Test
    fun theCapIsTwoPerRole() {
        assertEquals("1", pairings.freeId(Role.PARENT))
        pairings.store(Role.PARENT, "1").save(pairing("one"))
        assertEquals("2", pairings.freeId(Role.PARENT))
        pairings.store(Role.PARENT, "2").save(pairing("two"))
        assertNull("no third pairing", pairings.freeId(Role.PARENT))
        assertEquals(listOf("1", "2"), pairings.savedIds(Role.PARENT))
        assertEquals("1", pairings.freeId(Role.BABY))
    }

    @Test
    fun theFirstSavedPairingIsTheDefaultUntilTheUserPicksAnother() {
        saveTwo()
        assertEquals("1", pairings.defaultId(Role.PARENT))
        assertTrue(pairings.setDefault(Role.PARENT, "2"))
        assertEquals("2", pairings.defaultId(Role.PARENT))
        assertEquals(listOf(false, true), pairings.list(Role.PARENT).map { it.isDefault })
        assertEquals("two", pairings[Role.PARENT].load()?.pskB64url)
        assertTrue(pairings.setDefault(Role.PARENT, "1"))
        assertEquals("1", pairings.defaultId(Role.PARENT))
        assertEquals(listOf(true, false), pairings.list(Role.PARENT).map { it.isDefault })
    }

    @Test
    fun aSavedPairingIsOnlyListedAsDefaultWhileTheUserMarkedIt() {
        saveTwo()
        assertNull(pairings.markedDefaultId(Role.PARENT))
        assertEquals(listOf(false, false), pairings.list(Role.PARENT).map { it.isDefault })
        pairings.setDefault(Role.PARENT, "2")
        assertEquals("2", pairings.markedDefaultId(Role.PARENT))
        pairings.clearDefault(Role.PARENT)
        assertNull(pairings.markedDefaultId(Role.PARENT))
        assertEquals("1", pairings.defaultId(Role.PARENT))
        assertEquals(listOf(false, false), pairings.list(Role.PARENT).map { it.isDefault })
    }

    @Test
    fun theBabyListKeepsTheImpliedDefault() {
        pairings.store(Role.BABY, "1").save(pairing("one"))
        pairings.store(Role.BABY, "2").save(pairing("two"))
        assertEquals(listOf(true, false), pairings.list(Role.BABY).map { it.isDefault })
        pairings.setDefault(Role.BABY, "2")
        assertEquals(listOf(false, true), pairings.list(Role.BABY).map { it.isDefault })
    }

    @Test
    fun forgettingTheDefaultLeavesTheOtherAsDefault() {
        saveTwo()
        pairings.setDefault(Role.PARENT, "1")
        pairings.forget(Role.PARENT, "1")
        assertEquals(listOf("2"), pairings.savedIds(Role.PARENT))
        assertEquals("2", pairings.defaultId(Role.PARENT))
        assertEquals("two", pairings.store(Role.PARENT, "2").load()?.pskB64url)
    }

    @Test
    fun forgetDeletesExactlyOnePairingAndItsKey() {
        saveTwo()
        pairings.store(Role.BABY, "1").save(pairing("baby"))
        pairings.forget(Role.PARENT, "2")
        assertTrue(ciphers.getValue(Role.PARENT)[1].keyDeleted)
        assertFalse(ciphers.getValue(Role.PARENT)[0].keyDeleted)
        assertEquals("one", pairings.store(Role.PARENT, "1").load()?.pskB64url)
        assertNull(pairings.store(Role.PARENT, "2").load())
        assertEquals("baby", pairings.store(Role.BABY, "1").load()?.pskB64url)
        assertEquals("2", pairings.freeId(Role.PARENT))
    }

    @Test
    fun renameKeepsTheNameWithThePairingAndChecksTheLength() {
        saveTwo()
        assertTrue(pairings.rename(Role.PARENT, "2", "  Home – old phone "))
        assertEquals("Home – old phone", pairings.list(Role.PARENT)[1].name)
        assertNull(pairings.list(Role.PARENT)[0].name)
        assertEquals(
            "the secrets are untouched",
            "two",
            pairings.store(Role.PARENT, "2").load()?.pskB64url,
        )
        assertFalse("empty", pairings.rename(Role.PARENT, "1", "   "))
        assertFalse("31 characters", pairings.rename(Role.PARENT, "1", "x".repeat(31)))
        assertTrue("30 characters", pairings.rename(Role.PARENT, "1", "y".repeat(30)))
        assertFalse("no pairing in the slot", pairings.rename(Role.BABY, "2", "Nope"))
    }

    @Test
    fun theNameIsSealedWithThePairingNotLeftInPlainText() {
        pairings.store(Role.PARENT, "1").save(pairing("one"))
        pairings.rename(Role.PARENT, "1", "Granny's place")
        val raw = File(tmp.root, "pairing-parent-0.bin").readText(Charsets.ISO_8859_1)
        assertFalse(raw.contains("Granny"))
    }

    @Test
    fun aPeerDeviceNameIsOnlyTheDefaultName() {
        pairings.store(Role.BABY, "1").save(pairing("one"))
        pairings.nameIfUnset(Role.BABY, "1", "  Pixel 7  ")
        assertEquals("Pixel 7", pairings.list(Role.BABY)[0].name)
        pairings.nameIfUnset(Role.BABY, "1", "Other")
        assertEquals("Pixel 7", pairings.list(Role.BABY)[0].name)
        assertEquals("a".repeat(30), ConnectionName.fromPeerDevice("a".repeat(45)))
        assertNull(ConnectionName.fromPeerDevice("  "))
    }

    @Test
    fun theNameNeverReachesToString() {
        saveTwo()
        pairings.rename(Role.PARENT, "1", "Secret-Name")
        val texts =
            listOf(
                pairings.store(Role.PARENT, "1").load().toString(),
                pairings.list(Role.PARENT).toString(),
                pairings.list(Role.PARENT)[0].toString(),
                ConnectionUi("1", "Secret-Name", isDefault = true, selected = true).toString(),
            )
        texts.forEach { assertFalse(it, it.contains("Secret-Name")) }
    }

    @Test
    fun theDefaultSurvivesARestartWithoutReadingASealedRecord() {
        val dir = tmp.newFolder("marker")
        fun build(cipher: org.guardedsleep.app.pairing.PairingCipher) =
            RolePairings(
                Role.entries.associateWith { role ->
                    listOf("a", "b").map {
                        PairingStore(File(dir, "p-${role.name.lowercase()}-$it.bin"), cipher)
                    }
                },
                defaults = org.guardedsleep.app.pairing.DefaultPairingMarker.Files(dir),
            )
        val first = build(FakeCipher())
        first.store(Role.PARENT, "1").save(pairing("one"))
        first.store(Role.PARENT, "2").save(pairing("two"))
        first.setDefault(Role.PARENT, "2")

        // A busy Keystore makes every record unreadable; the default is still known.
        val busy = build(FlakyCipher(100) { IllegalStateException("keystore busy") })
        assertEquals("2", busy.defaultId(Role.PARENT))
    }

    /** Holds the first decrypt after [arm] until [release], so a second writer can interleave. */
    private class GateCipher : org.guardedsleep.app.pairing.PairingCipher {
        private val inner = FakeCipher()
        private val armed = java.util.concurrent.atomic.AtomicBoolean(false)
        val inLoad = java.util.concurrent.CountDownLatch(1)
        val release = java.util.concurrent.CountDownLatch(1)

        fun arm() = armed.set(true)

        override fun encrypt(plain: ByteArray) = inner.encrypt(plain)

        override fun decrypt(sealed: ByteArray): ByteArray {
            val plain = inner.decrypt(sealed)
            if (armed.compareAndSet(true, false)) {
                inLoad.countDown()
                check(release.await(10, java.util.concurrent.TimeUnit.SECONDS))
            }
            return plain
        }

        override fun deleteKey() = inner.deleteKey()
    }

    @Test
    fun aRenameNeverOverwritesAConcurrentParentSecret() {
        val cipher = GateCipher()
        val rp =
            RolePairings(
                Role.entries.associateWith { role ->
                    listOf("a", "b").map {
                        PairingStore(
                            File(tmp.root, "race-${role.name.lowercase()}-$it.bin"),
                            cipher,
                        )
                    }
                }
            )
        val store = rp.store(Role.PARENT, "1")
        store.save(pairing("one"))

        cipher.arm()
        val renamer = Thread { rp.rename(Role.PARENT, "1", "Home") }
        renamer.start()
        assertTrue(cipher.inLoad.await(10, java.util.concurrent.TimeUnit.SECONDS))
        // The core thread writes the secret while the rename sits between its load and its save.
        val writer = Thread { store.setParentSecret("secret-b64", 123L) }
        writer.start()
        Thread.sleep(200)
        cipher.release.countDown()
        renamer.join(10_000)
        writer.join(10_000)

        val after = store.load()
        assertEquals("Home", after?.connectionName)
        assertEquals("secret-b64", after?.parentSecretB64url)
        assertEquals(123L, after?.pinUntilS)
    }

    @Test
    fun theDeviceNameNeverReplacesAChosenName() {
        pairings.store(Role.BABY, "1").save(pairing("one"))
        pairings.rename(Role.BABY, "1", "Mine")
        pairings.nameIfUnset(Role.BABY, "1", "Pixel")
        assertEquals("Mine", pairings.list(Role.BABY)[0].name)
    }
}
