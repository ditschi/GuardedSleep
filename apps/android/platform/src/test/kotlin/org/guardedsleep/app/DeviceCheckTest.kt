package org.guardedsleep.app

import org.guardedsleep.app.readiness.DeviceCheckEnv
import org.guardedsleep.app.readiness.DeviceCheckRules
import org.guardedsleep.app.readiness.DeviceChecker
import org.guardedsleep.app.readiness.DeviceFacts
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.DeviceCheckReason
import org.guardedsleep.app.ui.model.MailboxReach
import org.junit.Assert.assertEquals
import org.junit.Test

class DeviceCheckTest {
    private val good =
        DeviceFacts(
            micGranted = true,
            micRms = 0.01,
            batteryPct = 90,
            charging = true,
            batteryUnhealthy = false,
            batteryOptimizationIgnored = true,
            dndAccessGranted = true,
            mailboxes = listOf(MailboxReach("a.example", true)),
        )

    private fun eval(f: DeviceFacts) = DeviceCheckRules.evaluate(f)

    @Test
    fun everythingFineIsOk() {
        val r = eval(good)
        assertEquals(DeviceCheck.OK, r.check)
        assertEquals(emptyList<DeviceCheckReason>(), r.reasons)
    }

    @Test
    fun missingMicPermissionIsNotRecommended() {
        val r = eval(good.copy(micGranted = false, micRms = null))
        assertEquals(DeviceCheck.NOT_RECOMMENDED, r.check)
        assertEquals(listOf(DeviceCheckReason.MIC_PERMISSION_MISSING), r.reasons)
    }

    @Test
    fun digitalSilenceIsNotRecommended() {
        val r = eval(good.copy(micRms = 0.0))
        assertEquals(DeviceCheck.NOT_RECOMMENDED, r.check)
        assertEquals(listOf(DeviceCheckReason.MIC_SILENT), r.reasons)
    }

    @Test
    fun aSkippedMicSampleAddsNoReason() {
        assertEquals(DeviceCheck.OK, eval(good.copy(micRms = null)).check)
    }

    @Test
    fun unhealthyBatteryIsNotRecommended() {
        val r = eval(good.copy(batteryUnhealthy = true))
        assertEquals(DeviceCheck.NOT_RECOMMENDED, r.check)
        assertEquals(listOf(DeviceCheckReason.BATTERY_UNHEALTHY), r.reasons)
    }

    @Test
    fun lowBatteryThatDoesNotChargeIsNotRecommendedAndOnlyThatReason() {
        val r = eval(good.copy(batteryPct = 12, charging = false))
        assertEquals(DeviceCheck.NOT_RECOMMENDED, r.check)
        assertEquals(listOf(DeviceCheckReason.BATTERY_LOW_NOT_CHARGING), r.reasons)
    }

    @Test
    fun lowBatteryWhileChargingIsFine() {
        assertEquals(DeviceCheck.OK, eval(good.copy(batteryPct = 5, charging = true)).check)
    }

    @Test
    fun notChargingWithAGoodBatteryIsLimited() {
        val r = eval(good.copy(charging = false))
        assertEquals(DeviceCheck.LIMITED, r.check)
        assertEquals(listOf(DeviceCheckReason.NOT_CHARGING), r.reasons)
    }

    @Test
    fun batteryOptimizationIsLimited() {
        val r = eval(good.copy(batteryOptimizationIgnored = false))
        assertEquals(DeviceCheck.LIMITED, r.check)
        assertEquals(listOf(DeviceCheckReason.BATTERY_OPTIMIZED), r.reasons)
    }

    @Test
    fun missingDndAccessIsLimited() {
        val r = eval(good.copy(dndAccessGranted = false))
        assertEquals(DeviceCheck.LIMITED, r.check)
        assertEquals(listOf(DeviceCheckReason.DND_ACCESS_MISSING), r.reasons)
    }

    @Test
    fun someUnreachableMailboxesAreLimited() {
        val mailboxes = listOf(MailboxReach("a.example", true), MailboxReach("b.example", false))
        val r = eval(good.copy(mailboxes = mailboxes))
        assertEquals(DeviceCheck.LIMITED, r.check)
        assertEquals(listOf(DeviceCheckReason.SOME_MAILBOXES_UNREACHABLE), r.reasons)
        assertEquals(mailboxes, r.mailboxes)
    }

    @Test
    fun noReachableMailboxIsNotRecommended() {
        val mailboxes = listOf(MailboxReach("a.example", false), MailboxReach("b.example", false))
        val r = eval(good.copy(mailboxes = mailboxes))
        assertEquals(DeviceCheck.NOT_RECOMMENDED, r.check)
        assertEquals(listOf(DeviceCheckReason.NO_MAILBOX_REACHABLE), r.reasons)
    }

    @Test
    fun anEmptyMailboxSetAddsNoReason() {
        assertEquals(DeviceCheck.OK, eval(good.copy(mailboxes = emptyList())).check)
    }

    @Test
    fun reasonsAreWorstFirst() {
        val r =
            eval(
                good.copy(
                    micGranted = false,
                    micRms = null,
                    batteryUnhealthy = true,
                    batteryPct = 10,
                    charging = false,
                    batteryOptimizationIgnored = false,
                    dndAccessGranted = false,
                    mailboxes = listOf(MailboxReach("a.example", false)),
                )
            )
        assertEquals(
            listOf(
                DeviceCheckReason.MIC_PERMISSION_MISSING,
                DeviceCheckReason.BATTERY_UNHEALTHY,
                DeviceCheckReason.BATTERY_LOW_NOT_CHARGING,
                DeviceCheckReason.NO_MAILBOX_REACHABLE,
                DeviceCheckReason.BATTERY_OPTIMIZED,
                DeviceCheckReason.DND_ACCESS_MISSING,
            ),
            r.reasons,
        )
        assertEquals(DeviceCheck.NOT_RECOMMENDED, r.check)
    }

    private class Env(val reach: Map<String, Boolean>, val rms: Double? = 0.01) : DeviceCheckEnv {
        var micSampled = false

        override fun micGranted() = true

        override fun batteryPct() = 90

        override fun charging() = true

        override fun batteryUnhealthy() = false

        override fun batteryOptimizationIgnored() = true

        override fun dndAccessGranted() = true

        override fun sampleMicRms(ms: Long): Double? {
            micSampled = true
            return rms
        }

        override fun probeMailbox(url: String, timeoutMs: Long) = reach[url] ?: false
    }

    @Test
    fun theCheckerProbesEveryMailboxAndReportsOnlyTheHost() {
        val env =
            Env(mapOf("wss://a.example/signaling?x=1" to true, "wss://b.example:8443/s" to false))
        val r =
            DeviceChecker(env)
                .check(listOf("wss://a.example/signaling?x=1", "wss://b.example:8443/s"), true)
        assertEquals(
            listOf(MailboxReach("a.example", true), MailboxReach("b.example", false)),
            r.mailboxes,
        )
        assertEquals(DeviceCheck.LIMITED, r.check)
    }

    @Test
    fun theCheckerSkipsTheMicWhenToldTo() {
        val env = Env(emptyMap())
        DeviceChecker(env).check(emptyList(), sampleMic = false)
        assertEquals(false, env.micSampled)
    }
}
