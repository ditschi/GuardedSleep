package org.guardedsleep.app.journey

import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.Role

/** The user flows the journeys share, each a sequence of taps and waits by test tag. */

/** The role screen of [device]: choose [role] and finish the setup checklist. */
fun JourneyHost.chooseRoleAndFinishSetup(device: Device, role: Role) {
    awaitShown(device, "role_title")
    tap(device, if (role == Role.BABY) "role_baby" else "role_parent")
    awaitShown(device, "setup_title")
    tap(device, "setup_continue")
}

/**
 * First run on both phones with pairing by pasted code: the code is read from the baby's code text
 * node and typed into the parent's paste field, the path a user takes without a camera. Ends with
 * both phones monitoring and connected.
 */
fun JourneyHost.firstRunAndPair() {
    chooseRoleAndFinishSetup(Device.BABY, Role.BABY)
    chooseRoleAndFinishSetup(Device.PARENT, Role.PARENT)
    pairWithPastedCode()
    awaitConnected()
}

/** Baby shows its invite, parent pastes it (both already on their pairing screens). */
fun JourneyHost.pairWithPastedCode() {
    awaitShown(Device.BABY, "baby_pair")
    awaitShown(Device.PARENT, "parent_pair")
    tap(Device.BABY, "baby_pair_show_text")
    awaitShown(Device.BABY, "baby_pair_code_text")
    val code = text(Device.BABY, "baby_pair_code_text")
    type(Device.PARENT, "parent_pair_paste_field", code)
    tap(Device.PARENT, "parent_pair_paste_submit")
    awaitShown(Device.BABY, "baby_monitor")
    awaitShown(Device.PARENT, "parent_monitor")
}

/** Both phones monitor and the link is up on both sides. */
fun JourneyHost.awaitConnected() {
    stepUntil("both sides connected", timeoutMs = 30_000) {
        phones.baby.baby.value.link.phase == LinkPhase.CONNECTED &&
            phones.parent.parent.value.link.phase == LinkPhase.CONNECTED
    }
}

/** Stops monitoring through the stop button and its confirmation. */
fun JourneyHost.stopMonitoring(device: Device) {
    // The parent's stop sits in the "more" sheet.
    if (device == Device.PARENT) tap(device, "parent_overflow")
    val stop = if (device == Device.BABY) "baby_stop" else "parent_stop"
    // Sheets and dialogs animate in on the Compose clock.
    awaitShown(device, stop)
    tap(device, stop)
    awaitShown(device, "dialog_stop_confirm")
    tap(device, "dialog_stop_confirm")
    awaitShown(device, "home")
}

/** Starts monitoring from the home card. */
fun JourneyHost.startFromHome(device: Device) {
    awaitShown(device, "home_card")
    tap(device, "home_card")
    awaitShown(device, if (device == Device.BABY) "baby_monitor" else "parent_monitor")
}
