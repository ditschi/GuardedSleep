package org.guardedsleep.app.journey

import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Test

/** The harness itself: both phones render, and one role choice per device reaches its setup. */
class SmokeJourneyTest : JourneyHost() {
    @Test
    fun bothRolesAreChosenAndReachTheirSetup() {
        awaitShown(Device.BABY, "role_baby")
        awaitShown(Device.PARENT, "role_parent")

        tap(Device.BABY, "role_baby")
        awaitShown(Device.BABY, "setup_title")
        tap(Device.PARENT, "role_parent")
        awaitShown(Device.PARENT, "setup_title")

        assertEquals(Role.BABY, phones.baby.app.value.role)
        assertEquals(Role.PARENT, phones.parent.app.value.role)
    }
}
