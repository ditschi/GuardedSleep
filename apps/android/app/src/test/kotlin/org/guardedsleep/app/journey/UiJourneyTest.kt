package org.guardedsleep.app.journey

import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.FixMethodOrder
import org.junit.Ignore
import org.junit.Test
import org.junit.runners.MethodSorters

/**
 * The standard user flows on two real controllers (`ui-journey-tests`). All journeys live in one
 * class so the Robolectric start-up is paid once; the writer check runs last (method order by name)
 * and needs every journey to have run.
 */
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class UiJourneyTest : JourneyHost() {
    @Test
    fun firstRunPairsByPastedCodeAndBothMonitor() {
        firstRunAndPair()
        assertEquals(Role.BABY, phones.baby.app.value.role)
        assertTrue(phones.baby.app.value.paired)
        assertTrue(phones.parent.app.value.paired)
        assertTrue(phones.baby.app.value.monitoring)
        assertTrue(phones.parent.app.value.monitoring)
    }

    @Test
    fun startAndStopOnBothSides() {
        firstRunAndPair()
        stopMonitoring(Device.PARENT)
        stopMonitoring(Device.BABY)
        assertFalse(phones.baby.app.value.monitoring)
        assertFalse(phones.parent.app.value.monitoring)
        startFromHome(Device.BABY)
        startFromHome(Device.PARENT)
        awaitConnected()
        // The baby stops: the parent sees the deliberate stop, not a lost link.
        stopMonitoring(Device.BABY)
        stepUntil("the parent sees the baby stop") {
            phones.parent.parent.value.link.phase == LinkPhase.PEER_STOPPED
        }
    }

    @Test
    fun savedConnectionsDefaultAutomaticStartAndChooseAnother() {
        firstRunAndPair()
        // A second invite from the baby: the parent adds it as its second connection.
        stopMonitoring(Device.PARENT)
        stopMonitoring(Device.BABY)
        tap(Device.BABY, "home_new_pairing")
        tap(Device.PARENT, "home_new_pairing")
        pairWithPastedCode()
        awaitConnected()
        assertEquals(2, phones.parent.app.value.connections.size)
        stopMonitoring(Device.PARENT)
        stopMonitoring(Device.BABY)

        // Two saved connections: the list opens, one is chosen and marked as the default.
        tap(Device.PARENT, "home_connection_change")
        awaitShown(Device.PARENT, "home_connection_list")
        val chosen = phones.parent.app.value.connections.first { !it.selected }.id
        tap(Device.PARENT, "connection_choice_$chosen")
        assertEquals(chosen, phones.parent.app.value.connections.first { it.selected }.id)
        tap(Device.PARENT, "home_auto_connect")
        assertTrue(phones.parent.app.value.connections.first { it.selected }.isDefault)

        // The app starts again: the first entry into the parent role starts monitoring on its own.
        restart(Device.PARENT)
        awaitShown(Device.PARENT, "role_parent")
        tap(Device.PARENT, "role_parent")
        awaitShown(Device.PARENT, "parent_monitor")
        assertTrue(phones.parent.app.value.autoStarted)

        // Wrong place: one tap leaves it and lists the connections again.
        awaitShown(Device.PARENT, "parent_choose_other_connection")
        tap(Device.PARENT, "parent_choose_other_connection")
        awaitShown(Device.PARENT, "home_connection_list")
        assertFalse(phones.parent.app.value.monitoring)

        // Clearing the default through the switch.
        tap(Device.PARENT, "home_auto_connect")
        assertTrue(phones.parent.app.value.connections.none { it.isDefault })
    }

    @Test
    fun pairAgainAfterTheRelayRefusesTheLink() {
        // Both phones know a pairing that never verified its peer (a first attempt).
        chooseRoleAndFinishSetup(Device.BABY, Role.BABY)
        chooseRoleAndFinishSetup(Device.PARENT, Role.PARENT)
        tap(Device.BABY, "baby_pair_cancel")
        tap(Device.PARENT, "parent_pair_cancel")
        phones.seedUnverifiedPairing()
        restart(Device.BABY)
        restart(Device.PARENT)
        awaitShown(Device.BABY, "role_baby")
        tap(Device.BABY, "role_baby")
        awaitShown(Device.PARENT, "role_parent")
        tap(Device.PARENT, "role_parent")
        // The relay refuses the protocol: such a link ends in a terminal failure, and both
        // phones offer "Pair again".
        phones.relayRejectsProtocol(true)
        startFromHome(Device.BABY)
        startFromHome(Device.PARENT)
        awaitShown(Device.BABY, "baby_pair_again")
        awaitShown(Device.PARENT, "parent_pair_again")
        phones.relayRejectsProtocol(false)
        tap(Device.BABY, "baby_pair_again")
        tap(Device.PARENT, "parent_pair_again")
        pairWithPastedCode()
        awaitConnected()
    }

    @Test
    fun noiseAlertStopSoundSnoozeAndCancelSnooze() {
        firstRunAndPair()
        // The baby unit's own detection settings travel to its state.
        phones.baby.updateSettings { it.copy(threshold = 0.2f, holdSeconds = 3) }
        phones.injectNoise()
        stepUntil("the parent phone raises the noise alert", timeoutMs = 30_000) {
            phones.parentNoiseAlerts > 0
        }
        assertTrue(phones.parent.parent.value.noiseAlertActive)
        // The full-screen alert the notification opens.
        showAlert(AlertKind.NOISE)
        awaitShown(Device.PARENT, "alert_stop_sound")
        tap(Device.PARENT, "alert_stop_sound")
        assertEquals(1, phones.parentSoundStops)
        awaitShown(Device.PARENT, "alert_sound_stopped")
        tap(Device.PARENT, "alert_snooze_10")
        assertNotNull(phones.parent.parent.value.snoozedUntilWallMs)
        assertNotShown(Device.PARENT, "alert_stop_sound")
        // Cancelling the snooze from the monitor.
        tap(Device.PARENT, "parent_snooze")
        awaitShown(Device.PARENT, "sheet_snooze_end")
        tap(Device.PARENT, "sheet_snooze_end")
        assertNull(phones.parent.parent.value.snoozedUntilWallMs)
        // The noise stops: the event ends and the parent keeps the last sound.
        phones.stopNoise()
        // The core ends an event after two quiet minutes.
        stepUntil("the noise event ends", timeoutMs = 5 * 60_000L, stepMs = 1_000) {
            !phones.parent.parent.value.noiseAlertActive
        }
        assertNotNull(phones.parent.parent.value.lastSound)
    }

    @Test
    fun kioskLockAndUnlock() {
        firstRunAndPair()
        // The parent listens (the hold gesture of the button is not driven here) and asks the baby
        // unit to silence itself.
        phones.parent.setListen(ListenMode.LOCKED)
        stepUntil("the baby unit knows the parent listens") {
            phones.baby.baby.value.parentListening
        }
        tap(Device.BABY, "baby_lock")
        awaitShown(Device.BABY, "baby_kiosk")
        assertTrue(phones.baby.baby.value.kioskLocked)
        customAction(Device.BABY, "baby_kiosk")
        awaitShown(Device.BABY, "baby_monitor")
        assertFalse(phones.baby.baby.value.kioskLocked)
    }

    @Test
    fun linkLossAcknowledgeAndRecovery() {
        firstRunAndPair()
        phones.dropNetwork()
        // The baby hears something while nobody listens.
        phones.injectNoise()
        // The parent's alarm escalates while the link stays down; virtual time, no waiting.
        stepUntil("the link alarm escalates", timeoutMs = 120_000, stepMs = 500) {
            phones.parent.parent.value.link.level >= 2
        }
        assertTrue(phones.parentLinkLostAlarms > 0)
        showAlert(AlertKind.LINK)
        awaitShown(Device.PARENT, "link_alarm_ack")
        tap(Device.PARENT, "link_alarm_ack")
        assertNotNull(phones.parent.parent.value.link.acknowledgedUntilElapsedMs)
        phones.stopNoise()
        // Network back: both phones reconnect without a tap.
        phones.restoreNetwork()
        stepUntil("the link recovers", timeoutMs = 120_000, stepMs = 500) {
            phones.parent.parent.value.link.phase == LinkPhase.CONNECTED &&
                phones.baby.baby.value.link.phase == LinkPhase.CONNECTED
        }
        assertEquals(0, phones.parent.parent.value.link.level)
    }

    @Test
    fun deviceCheckResultAndCheckAgain() {
        awaitShown(Device.BABY, "role_baby")
        tap(Device.BABY, "role_baby")
        awaitShown(Device.BABY, "setup_title")
        stepUntil("the first device check finishes") {
            phones.baby.app.value.deviceCheck != DeviceCheck.UNKNOWN
        }
        assertEquals(DeviceCheck.OK, phones.baby.app.value.deviceCheck)
        // The mailbox cannot be reached: not recommended, with the reason on the screen.
        phones.babyDevice.reachable = false
        tap(Device.BABY, "setup_device_check_again")
        stepUntil("the new result") {
            phones.baby.app.value.deviceCheck == DeviceCheck.NOT_RECOMMENDED
        }
        awaitShown(Device.BABY, "setup_device_check_reason_no_mailbox_reachable")
        phones.babyDevice.reachable = true
        tap(Device.BABY, "setup_device_check_again")
        stepUntil("OK again") { phones.baby.app.value.deviceCheck == DeviceCheck.OK }
        assertNotShown(Device.BABY, "setup_device_check_reason_no_mailbox_reachable")
    }

    @Test
    fun aWrongPairingCodeIsRefusedAndTheRightOneWorks() {
        chooseRoleAndFinishSetup(Device.BABY, Role.BABY)
        chooseRoleAndFinishSetup(Device.PARENT, Role.PARENT)
        awaitShown(Device.PARENT, "parent_pair")
        type(Device.PARENT, "parent_pair_paste_field", "this is not a pairing code")
        tap(Device.PARENT, "parent_pair_paste_submit")
        awaitShown(Device.PARENT, "parent_pair_error")
        assertNotNull(phones.parent.pairing.value?.error)
        pairWithPastedCode()
        awaitConnected()
    }

    @Test
    fun testAlertReachesTheParentPhoneWithTheTestBadge() {
        firstRunAndPair()
        tap(Device.PARENT, "parent_probe")
        awaitShown(Device.PARENT, "sheet_probe_run")
        tap(Device.PARENT, "sheet_probe_run")
        stepUntil("the test alert arrives", timeoutMs = 30_000) {
            phones.parent.parent.value.noiseIsTest
        }
    }

    @Test
    fun switchModeBetweenTheRoles() {
        firstRunAndPair()
        stopMonitoring(Device.PARENT)
        stopMonitoring(Device.BABY)
        // The baby phone becomes a parent phone: the role screen, then the parent's own pairing
        // entry (this phone has no parent connection yet).
        tap(Device.BABY, "switch_mode")
        awaitShown(Device.BABY, "role_title")
        assertNull(phones.baby.app.value.role)
        tap(Device.BABY, "role_default_toggle")
        tap(Device.BABY, "role_parent")
        awaitShown(Device.BABY, "parent_pair")
        assertEquals(Role.PARENT, phones.baby.app.value.defaultRole)
        assertEquals(Role.PARENT, phones.baby.app.value.role)
        // Back to the baby role: its saved connection is untouched.
        tap(Device.BABY, "switch_mode")
        awaitShown(Device.BABY, "role_title")
        tap(Device.BABY, "role_baby")
        awaitShown(Device.BABY, "home_card")
        assertTrue(phones.baby.app.value.paired)
    }

    // ---- exposed product gaps: written, ignored until the owner decides (backlog) -----------

    @Ignore(
        "What the test alert answer does is not specified: docs/backlog.md IDEA \"What the test " +
            "alert answer does\" (ui-journey-tests)"
    )
    @Test
    fun testAlertAnswerHasAnEffect() {
        firstRunAndPair()
        tap(Device.PARENT, "parent_probe")
        awaitShown(Device.PARENT, "sheet_probe_run")
        tap(Device.PARENT, "sheet_probe_run")
        awaitShown(Device.PARENT, "sheet_probe_yes")
        tap(Device.PARENT, "sheet_probe_yes")
        // Expected once specified: the test counts as passed (a visible readiness state change).
        assertNotNull(phones.parent.settings.value.lastTestWallMs)
        assertNotEquals(0L, phones.parent.settings.value.lastTestWallMs)
    }

    @Ignore(
        "The first-run coach is only in alpha-ux-flows: docs/backlog.md IDEA about the first-run " +
            "coach (ui-journey-tests)"
    )
    @Test
    fun firstRunCoachIsShownAndDismissed() {
        firstRunAndPair()
        awaitShown(Device.PARENT, "coach_first_run")
        tap(Device.PARENT, "coach_later")
        assertNotShown(Device.PARENT, "coach_first_run")
        assertFalse(phones.parent.app.value.coachPending)
    }

    @Ignore(
        "Settings test alert while not monitoring does nothing: docs/backlog.md IDEA \"Settings " +
            "→ Test alert while not monitoring\" (ui-journey-tests)"
    )
    @Test
    fun settingsTestAlertWhileNotMonitoringSaysConnectFirst() {
        firstRunAndPair()
        stopMonitoring(Device.PARENT)
        tap(Device.PARENT, "home_settings")
        awaitShown(Device.PARENT, "settings_test_alert")
        tap(Device.PARENT, "settings_test_alert")
        // Expected once specified: a visible "connect first" message, not silence.
        assertShown(Device.PARENT, "settings_test_alert_connect_first")
    }

    // ---- the writer check (last) ------------------------------------------------------------

    /**
     * Every leaf field of the app, parent, baby and pairing state reached a non-default value in
     * some journey, or is on the allowlist with a reason. Settings are not checked: they are inputs
     * the user writes on the Settings screen, not platform-written state.
     */
    @Test
    fun zzEveryStateFieldIsWrittenByAJourney() {
        val journeys =
            UiJourneyTest::class.java.methods.filter {
                it.isAnnotationPresent(Test::class.java) &&
                    !it.isAnnotationPresent(Ignore::class.java) &&
                    !it.name.startsWith("zz")
            }
        // A filtered run or a failed journey leaves the record incomplete: that is reported by the
        // failed journey, not by this check.
        assumeTrue(
            "the writer check needs every journey to have passed in this run",
            StateRecorder.journeysPassed() >= journeys.size,
        )
        val unseen = StateRecorder.unseen(ROOTS).filterNot { it in WriterAllowlist.entries }
        assertEquals(
            "fields no journey ever wrote to a non-default value (write them or add an allowlist " +
                "entry with a reason): $unseen",
            emptyList<String>(),
            unseen,
        )
        val all = StateRecorder.allPaths(ROOTS)
        val seen = StateRecorder.seenPaths()
        assertEquals(
            "allowlist entries for fields that no longer exist",
            emptyList<String>(),
            WriterAllowlist.entries.keys.filter { it !in all },
        )
        assertEquals(
            "allowlisted fields a journey now writes (remove the entry)",
            emptyList<String>(),
            WriterAllowlist.entries.keys.filter { it in seen },
        )
    }

    private companion object {
        val ROOTS =
            listOf(
                AppState::class.java,
                ParentMonitorState::class.java,
                BabyMonitorState::class.java,
                PairingState::class.java,
            )
    }
}
