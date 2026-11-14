@file:OptIn(ExperimentalTestApi::class)

package org.guardedsleep.app.journey

import android.Manifest
import android.app.Application
import android.view.Gravity
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.SemanticsNodeInteractionCollection
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.text.AnnotatedString
import androidx.test.core.app.ApplicationProvider
import java.util.TimeZone
import org.guardedsleep.app.fixtures.TestPhones
import org.guardedsleep.app.ui.GuardedSleepRoot
import org.guardedsleep.app.ui.LinkAlarmScreen
import org.guardedsleep.app.ui.NoiseAlertScreen
import org.guardedsleep.app.ui.common.BatteryReading
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.UiEnv
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.screens.LocalScannerView
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowLog

/** Which of the two phones a step talks to. */
enum class Device(val tag: String) {
    BABY("device_baby"),
    PARENT("device_parent"),
}

/** The full-screen alerts a phone opens over everything else (`AlertActivity`). */
enum class AlertKind {
    NOISE,
    LINK,
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
 *
 * What stays outside (Robolectric cannot prove it, the emulator gate does): launching
 * `AlertActivity` from a notification, the foreground service, Doze, real audio and the camera.
 * [showAlert] stands in for the activity launch: a journey opens the alert screen when the platform
 * raised the alert, exactly as the notification's full-screen intent would.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
// Wide enough for two phones side by side; the qualifiers keep Pixel 6 density.
@Config(sdk = [35], qualifiers = "w1600dp-h900dp-xxhdpi")
abstract class JourneyHost {
    @get:Rule val tmp = TemporaryFolder()

    // Counted once the journey and its teardown passed (the writer check needs them all).
    @get:Rule
    val outcome: org.junit.rules.TestWatcher =
        object : org.junit.rules.TestWatcher() {
            override fun succeeded(description: org.junit.runner.Description?) {
                StateRecorder.journeyPassed()
            }
        }
    @get:Rule
    val compose: AndroidComposeTestRule<*, ComponentActivity> =
        createAndroidComposeRule<ComponentActivity>()

    lateinit var phones: TestPhones
        private set

    private var startedAtMs = 0L

    // What each ComposeView renders: the controller (replaced by a restart) and an open alert.
    private var babyController by mutableStateOf<AppController?>(null)
    private var parentController by mutableStateOf<AppController?>(null)
    private var parentAlert by mutableStateOf<AlertKind?>(null)

    @Before
    fun hostSetUp() {
        ShadowLog.clear()
        // The setup checklist reads the real permission state: the baby's required row is the mic.
        shadowOf(ApplicationProvider.getApplicationContext<Application>())
            .grantPermissions(Manifest.permission.RECORD_AUDIO)
        phones = TestPhones.create(tmp.newFolder("phones"))
        babyController = phones.baby
        parentController = phones.parent
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
            fun phoneView(content: @Composable () -> Unit, gravity: Int) =
                ComposeView(compose.activity).apply {
                    setContent(content)
                    layout.addView(
                        this,
                        FrameLayout.LayoutParams(
                            half,
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            gravity,
                        ),
                    )
                }
            phoneView(
                { babyController?.let { PhoneRoot(Device.BABY, it, env, null) {} } },
                Gravity.START,
            )
            phoneView(
                {
                    parentController?.let {
                        PhoneRoot(Device.PARENT, it, env, parentAlert) { parentAlert = null }
                    }
                },
                Gravity.END,
            )
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
        record()
    }

    /** Feeds the states both phones show to the [StateRecorder] (the writer check). */
    private fun record() {
        listOf(phones.baby, phones.parent).forEach {
            StateRecorder.sample(it.app.value)
            StateRecorder.sample(it.parent.value)
            StateRecorder.sample(it.baby.value)
            StateRecorder.sample(it.pairing.value)
        }
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
            if (waited >= timeoutMs) {
                throw AssertionError(
                    "timeout waiting for $what; on screen: " +
                        "baby ${tagsOn(Device.BABY)} parent ${tagsOn(Device.PARENT)}; " +
                        "baby link ${phones.baby.baby.value.link}; " +
                        "parent link ${phones.parent.parent.value.link}; opened ${phones.openedUrls}; " +
                        "recent log: " +
                        ShadowLog.getLogs().takeLast(12).joinToString(" | ") { "${it.msg}" }
                )
            }
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

    // Sheets and dialogs are windows of their own: outside both device roots, so a node there
    // belongs to whichever phone opened it (only one does at a time in a journey).
    private fun inWindow(tag: String): SemanticsMatcher =
        hasTestTag(tag) and
            !hasAnyAncestor(hasTestTag(Device.BABY.tag) or hasTestTag(Device.PARENT.tag))

    private fun matcher(device: Device, tag: String): SemanticsMatcher =
        if (compose.onAllNodes(tagged(device, tag)).fetchSemanticsNodes().isNotEmpty())
            tagged(device, tag)
        else inWindow(tag)

    fun node(device: Device, tag: String): SemanticsNodeInteraction =
        compose.onNode(matcher(device, tag))

    fun nodes(device: Device, tag: String): SemanticsNodeInteractionCollection =
        compose.onAllNodes(matcher(device, tag))

    fun isShown(device: Device, tag: String) = nodes(device, tag).fetchSemanticsNodes().isNotEmpty()

    /** The test tags on screen on [device] (for failure messages). */
    fun tagsOn(device: Device): List<String> =
        compose
            .onAllNodes(within(device))
            .fetchSemanticsNodes()
            .mapNotNull { it.config.getOrNull(SemanticsProperties.TestTag) }
            .distinct()

    /**
     * Taps through the node's click action (no pointer geometry, so a row below the fold works the
     * same as on a phone where the user scrolls). A disabled button has no action and fails.
     */
    fun tap(device: Device, tag: String) {
        node(device, tag).performSemanticsAction(SemanticsActions.OnClick)
        step(0)
    }

    /** Sets the text of an input field the way typing or pasting does. */
    fun type(device: Device, tag: String, text: String) {
        node(device, tag).performSemanticsAction(SemanticsActions.SetText) {
            it(AnnotatedString(text))
        }
        step(0)
    }

    /** The text a node shows (all its text pieces joined). */
    fun text(device: Device, tag: String): String =
        node(device, tag)
            .fetchSemanticsNode()
            .config
            .getOrNull(SemanticsProperties.Text)
            .orEmpty()
            .joinToString("") { it.text }

    /** True when the node has a click action, i.e. an enabled button. */
    fun isEnabled(device: Device, tag: String) =
        node(device, tag).fetchSemanticsNode().config.contains(SemanticsActions.OnClick)

    fun awaitShown(device: Device, tag: String, timeoutMs: Long = 15_000) =
        stepUntil("$tag on ${device.tag}", timeoutMs) { isShown(device, tag) }

    fun awaitGone(device: Device, tag: String, timeoutMs: Long = 15_000) =
        stepUntil("$tag gone from ${device.tag}", timeoutMs) { !isShown(device, tag) }

    fun assertShown(device: Device, tag: String) =
        assertTrue(
            "$tag is not shown on ${device.tag}; on screen: ${tagsOn(device)}",
            isShown(device, tag),
        )

    fun assertNotShown(device: Device, tag: String) =
        assertFalse("$tag is shown on ${device.tag}", isShown(device, tag))

    fun assertText(device: Device, tag: String, expected: String) =
        assertEquals(expected, text(device, tag))

    /** Runs the node's custom accessibility action (the kiosk's "unlock" one). */
    fun customAction(device: Device, tag: String, index: Int = 0) {
        val actions = node(device, tag).fetchSemanticsNode().config[SemanticsActions.CustomActions]
        compose.runOnUiThread { actions[index].action() }
        step(0)
    }

    // ---- what a phone does outside the composition ------------------------------------------

    /** The app of [device] starts again over the files it saved (see [TestPhones.restartBaby]). */
    fun restart(device: Device) {
        when (device) {
            Device.BABY -> {
                phones.restartBaby()
                babyController = phones.baby
            }
            Device.PARENT -> {
                phones.restartParent()
                parentController = phones.parent
                parentAlert = null
            }
        }
        step(0)
    }

    /** The parent phone opens the full-screen alert the platform raised (see the class note). */
    fun showAlert(kind: AlertKind) {
        parentAlert = kind
        step(0)
    }

    companion object {
        /** Wall-clock budget of one journey (the suite shares 90 s). */
        const val JOURNEY_LIMIT_MS = 60_000L
        private const val MIN_FRAME_MS = 16L
    }
}

@Composable
private fun PhoneRoot(
    device: Device,
    controller: AppController,
    env: UiEnv,
    alert: AlertKind?,
    onCloseAlert: () -> Unit,
) {
    CompositionLocalProvider(
        LocalUiEnv provides env,
        // The camera is out of scope; the pairing code is pasted (QrScanTest covers the decoder).
        LocalScannerView provides { _, _ -> },
    ) {
        // A restart builds a new composition: nothing survives from the old controller.
        key(controller) {
            Box(Modifier.testTag(device.tag)) {
                GuardedSleepRoot(controller)
                when (alert) {
                    AlertKind.NOISE ->
                        Box(Modifier.fillMaxSize()) { NoiseAlertScreen(controller, onCloseAlert) }
                    AlertKind.LINK ->
                        Box(Modifier.fillMaxSize()) { LinkAlarmScreen(controller, onCloseAlert) }
                    null -> Unit
                }
            }
        }
    }
}
