package org.guardedsleep.app.journey

import androidx.test.core.app.ApplicationProvider
import org.guardedsleep.app.AlertActionReceiver
import org.guardedsleep.app.AlertActivity
import org.guardedsleep.app.GuardedSleepApp
import org.guardedsleep.app.MainActivity
import org.guardedsleep.app.ui.model.Role
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The manual dependency graph (`AppGraph`) that `MainActivity` uses builds under Robolectric with
 * the real settings store, pairing files, alert controller, readiness checker and controller, and
 * the real controller answers a role choice. The journeys build their own controllers around fakes,
 * so this is the only test that exercises the wiring in `app`. What it cannot prove stays with the
 * emulator gate: the Keystore, libwebrtc, the foreground service and the notification channels.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class AppGraphSmokeTest {
    @Test
    fun theGraphBuildsAndTheRealControllerAnswers() {
        val app = ApplicationProvider.getApplicationContext<GuardedSleepApp>()
        val graph = app.graph
        val controller = graph.controller
        // The platform services and the activities the notifications open are wired.
        assertSame(controller, app.services.controller)
        assertSame(MainActivity::class.java, graph.entryPoints.mainActivity)
        assertSame(AlertActivity::class.java, graph.entryPoints.alertActivity)
        assertSame(AlertActionReceiver::class.java, graph.entryPoints.alertActionReceiver)
        // A fresh install starts on the role screen, and a role choice reaches the store.
        assertNull(controller.app.value.role)
        assertFalse(controller.app.value.paired)
        controller.chooseRole(Role.BABY, false)
        assertEquals(Role.BABY, controller.app.value.role)
        controller.completeOnboarding()
        assertTrue(controller.app.value.onboardingDone)
        assertNotNull(graph.settings.current)
        assertEquals(Role.BABY, graph.settings.current.role)
    }
}
