@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.journey

import android.view.Gravity
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.SemanticsNodeInteractionCollection
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import java.util.TimeZone
import org.guardedsleep.app.fixtures.TestPhones
import org.guardedsleep.app.ui.GuardedSleepRoot
import org.guardedsleep.app.ui.common.BatteryReading
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.UiEnv
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.screens.LocalScannerView
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Rule
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowLog

/** Which of the two phones a step talks to. */
enum class Device(val tag: String) {
    BABY("device_baby"),
    PARENT("device_parent"),
}

/**
 * Two phones side by side in one activity (`ui-journey-tests`): each is a `ComposeView` whose root
 * carries `device_baby` / `device_parent` and renders [GuardedSleepRoot] on its own real controller
 * (through [TestPhones]), so UI-local state (overlays, half-typed fields, dialogs) behaves as on
 * two phones. Journeys select nodes by stable test tag within one device, never by copy text.
 *
 * Time is virtual: [step] advances the controllers' scheduler and the Compose clock together
 * (`mainClock.autoAdvance = false`), so a 60 s link loss costs no wall time. Every journey has a
 * wall-clock limit ([JOURNEY_LIMIT_MS]) checked in the step helpers; a blocking call that never
 * returns is not caught by it and would show up as a hang of the suite.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
// Wide enough for two phones side by side; the qualifiers keep Pixel 6 density.
@Config(sdk = [35], qualifiers = "w1600dp-h900dp-xxhdpi")
abstract class JourneyHost {
    @get:Rule val tmp = TemporaryFolder()
    @get:Rule
    val compose: AndroidComposeTestRule<*, ComponentActivity> =
        createAndroidComposeRule<ComponentActivity>()

    protected lateinit var phones: TestPhones
        private set

    private var startedAtMs = 0L

    @Before
    fun hostSetUp() {
        ShadowLog.clear()
        phones = TestPhones.create(tmp.newFolder("phones"))
        // Compose's clock is driven by the same steps as the controllers' scheduler.
        compose.mainClock.autoAdvance = false
        val env =
            UiEnv(
                elapsedNow = phones::elapsedMs,
                wallNow = phones::wallMs,
                ticking = true,
                reduceMotion = true,
                zone = TimeZone.getTimeZone("UTC"),
                cameraGranted = true,
                battery = BatteryReading(82, true),
            )
        compose.runOnUiThread {
            // A FrameLayout with exact widths: a weighted LinearLayout measures the ComposeViews
            // with unbounded height, which scrolling screens refuse.
            val layout = FrameLayout(compose.activity)
            val half = compose.activity.resources.displayMetrics.widthPixels / 2
            fun phoneView(device: Device, controller: AppController, gravity: Int) =
                ComposeView(compose.activity).apply {
                    setContent { PhoneRoot(device, controller, env) }
                    layout.addView(
                        this,
                        FrameLayout.LayoutParams(
                            half,
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            gravity,
                        ),
                    )
                }
            phoneView(Device.BABY, phones.baby, Gravity.START)
            phoneView(Device.PARENT, phones.parent, Gravity.END)
            compose.activity.setContentView(layout)
        }
        startedAtMs = System.currentTimeMillis()
        step(0)
    }

    @After
    fun hostTearDown() {
        phones.close()
        // The pairing secret never reaches a log line, in any encoding it could take.
        val logs =
            ShadowLog.getLogs().joinToString("\n") { "${it.tag} ${it.msg} ${it.throwable ?: ""}" }
        phones.pskForms.forEach { form ->
            // The message names the form's length only: the secret itself must not be printed.
            assertFalse(
                "a log line carries the pairing secret (a ${form.length}-char form)",
                logs.contains(form),
            )
        }
    }

    // ---- stepping ---------------------------------------------------------------------------

    /** Moves virtual time by [ms] on the controllers and on Compose, then lets the UI settle. */
    fun step(ms: Long = 0) {
        checkLimit()
        phones.advance(ms)
        compose.mainClock.advanceTimeBy(ms.coerceAtLeast(MIN_FRAME_MS))
        compose.waitForIdle()
    }

    /** Steps in [stepMs] slices until [cond] holds; fails after [timeoutMs] of virtual time. */
    fun stepUntil(
        what: String,
        timeoutMs: Long = 15_000,
        stepMs: Long = 50,
        cond: () -> Boolean,
    ) {
        var waited = 0L
        while (!cond()) {
            if (waited >= timeoutMs) throw AssertionError("timeout waiting for $what")
            step(stepMs)
            waited += stepMs
        }
    }

    private fun checkLimit() {
        if (System.currentTimeMillis() - startedAtMs > JOURNEY_LIMIT_MS) {
            throw AssertionError("journey exceeded its wall-clock limit of $JOURNEY_LIMIT_MS ms")
        }
    }

    // ---- device-scoped node access ----------------------------------------------------------

    private fun within(device: Device) = hasAnyAncestor(hasTestTag(device.tag))

    private fun tagged(device: Device, tag: String): SemanticsMatcher =
        hasTestTag(tag) and within(device)

    fun node(device: Device, tag: String): SemanticsNodeInteraction =
        compose.onNode(tagged(device, tag))

    fun nodes(device: Device, tag: String): SemanticsNodeInteractionCollection =
        compose.onAllNodes(tagged(device, tag))

    fun isShown(device: Device, tag: String) = nodes(device, tag).fetchSemanticsNodes().isNotEmpty()

    fun tap(device: Device, tag: String) {
        node(device, tag).performClick()
        step(0)
    }

    fun type(device: Device, tag: String, text: String) {
        node(device, tag).performTextInput(text)
        step(0)
    }

    fun awaitShown(device: Device, tag: String, timeoutMs: Long = 15_000) =
        stepUntil("$tag on ${device.tag}", timeoutMs) { isShown(device, tag) }

    fun assertNotShown(device: Device, tag: String) = nodes(device, tag).assertCountEquals(0)

    companion object {
        /** Wall-clock budget of one journey (the suite shares 90 s). */
        const val JOURNEY_LIMIT_MS = 60_000L
        private const val MIN_FRAME_MS = 16L
    }
}

@Composable
private fun PhoneRoot(device: Device, controller: AppController, env: UiEnv) {
    CompositionLocalProvider(
        LocalUiEnv provides env,
        // The camera is out of scope; the pairing code is pasted (QrScanTest covers the decoder).
        LocalScannerView provides { _, _ -> },
    ) {
        Box(Modifier.testTag(device.tag)) { GuardedSleepRoot(controller) }
    }
}
