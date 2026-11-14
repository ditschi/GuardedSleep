package org.guardedsleep.app.journey

import org.guardedsleep.app.fixtures.TestPhones
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Spike of `ui-journey-tests` task 2.0: two real cores (loaded through JNA inside the Robolectric
 * sandbox) pair and link through the fixtures, from `app` tests that see contract types only.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class RealCoreSpikeTest {
    @get:Rule val tmp = TemporaryFolder()

    @Test
    fun twoRealCoresPairAndConnect() {
        val phones = TestPhones.create(tmp.newFolder("phones"))
        try {
            phones.baby.chooseRole(Role.BABY, false)
            phones.parent.chooseRole(Role.PARENT, false)
            phones.baby.startPairing()
            val code = phones.baby.pairing.value!!.codePayload!!
            phones.parent.submitPairingCode(code)
            try {
                phones.runUntil("both paired") {
                    phones.baby.pairing.value?.phase == PairingPhase.PAIRED &&
                        phones.parent.pairing.value?.phase == PairingPhase.PAIRED
                }
            } catch (e: AssertionError) {
                throw AssertionError(
                    "baby=${phones.baby.pairing.value} / ${phones.baby.baby.value.link}; " +
                        "parent=${phones.parent.pairing.value} / ${phones.parent.parent.value.link}\n" +
                        org.robolectric.shadows.ShadowLog.getLogs().takeLast(40).joinToString(
                            "\n"
                        ) {
                            "${it.tag}: ${it.msg}"
                        },
                    e,
                )
            }
            phones.runUntil("both connected") {
                phones.baby.baby.value.link.phase == LinkPhase.CONNECTED &&
                    phones.parent.parent.value.link.phase == LinkPhase.CONNECTED
            }
            assertEquals(true, phones.baby.app.value.paired)
        } finally {
            phones.close()
        }
    }
}
