package org.guardedsleep.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import org.guardedsleep.app.BuildConfig
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.StatusBar
import org.guardedsleep.app.ui.common.findActivity
import org.guardedsleep.app.ui.common.systemReduceMotion
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.AlarmTonePickerScreen
import org.guardedsleep.app.ui.screens.BabyKioskScreen
import org.guardedsleep.app.ui.screens.BabyMonitorScreen
import org.guardedsleep.app.ui.screens.BuildInfo
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.LinkAlarmContent
import org.guardedsleep.app.ui.screens.NoiseAlertContent
import org.guardedsleep.app.ui.screens.PairBabyScreen
import org.guardedsleep.app.ui.screens.PairParentScreen
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.screens.ParentSheet
import org.guardedsleep.app.ui.screens.RoleScreen
import org.guardedsleep.app.ui.screens.SettingsPage
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.screens.SetupScreen
import org.guardedsleep.app.ui.screens.customPeerName
import org.guardedsleep.app.ui.theme.AppScheme
import org.guardedsleep.app.ui.theme.GuardedSleepTheme

/** Whole app below MainActivity: role, setup, pairing, home, monitoring, settings. */
@Composable
fun GuardedSleepRoot(controller: AppController) {
    val app by controller.app.collectAsState()
    val settings by controller.settings.collectAsState()
    val env = LocalUiEnv.current
    val systemReduce = systemReduceMotion()
    CompositionLocalProvider(
        LocalUiEnv provides env.copy(reduceMotion = env.reduceMotion || systemReduce)
    ) {
        // The baby phone is always dark (dark-room rules); the parent follows the theme setting.
        // Night mode (Ember) is resolved by the platform; the alert wrapper below never takes it.
        val night by controller.nightActive.collectAsState(false)
        GuardedSleepTheme(
            settings.theme,
            forceDark = app.role == Role.BABY,
            scheme = if (night) AppScheme.EMBER else null,
        ) {
            AppNavigator(controller)
        }
    }
}

private enum class Overlay {
    NONE,
    SETTINGS,
    ALARM_TONE,
    SETUP,
}

/**
 * Screen choice is a pure function of [AppState]/pairing so process restarts and the platform layer
 * (which owns `monitoring`) can never disagree with the UI. Only settings/setup are local overlays.
 */
@Composable
private fun AppNavigator(controller: AppController) {
    val app by controller.app.collectAsState()
    val pairing by controller.pairing.collectAsState()
    val baby by controller.baby.collectAsState()
    var overlay by rememberSaveable { mutableStateOf(Overlay.NONE) }
    // The Settings page survives the alarm tone picker and the set-up check, which replace it.
    var settingsPage by rememberSaveable { mutableStateOf(SettingsPage.ROOT) }
    var openProbe by remember { mutableStateOf(false) }
    val role = app.role
    // One-shot: "Choose another connection" asks the next home to show its connection list.
    var listRequested by rememberSaveable { mutableStateOf(false) }
    // Lives above the home so a cancelled auto-scan does not reopen; a new role or a saved
    // connection starts a fresh entry.
    var autoScanned by rememberSaveable(role, app.connections.isEmpty()) { mutableStateOf(false) }
    // Any open scan screen (the auto-scan itself or the home button) counts as this entry's scan,
    // so cancelling it does not rescan.
    LaunchedEffect(pairing != null) { if (pairing != null) autoScanned = true }

    when {
        role == null -> RoleScreen(controller)
        role == Role.BABY && app.monitoring && baby.kioskLocked -> BabyKioskScreen(controller)
        !app.onboardingDone -> SetupScreen(controller, role, onboarding = true, onBack = {})
        overlay == Overlay.SETUP ->
            WithStatusBar(controller) {
                SetupScreen(
                    controller,
                    role,
                    onboarding = false,
                    onBack = { overlay = Overlay.SETTINGS },
                )
            }
        overlay == Overlay.ALARM_TONE ->
            WithStatusBar(controller) {
                AlarmTonePickerScreen(controller, onBack = { overlay = Overlay.SETTINGS })
            }
        overlay == Overlay.SETTINGS ->
            WithStatusBar(controller) {
                SettingsScreen(
                    controller,
                    onOpenAlarmTone = { overlay = Overlay.ALARM_TONE },
                    onBack = {
                        settingsPage = SettingsPage.ROOT
                        overlay = Overlay.NONE
                    },
                    onOpenSetup = { overlay = Overlay.SETUP },
                    onTestAlert = {
                        openProbe = true
                        settingsPage = SettingsPage.ROOT
                        overlay = Overlay.NONE
                    },
                    initialPage = settingsPage,
                    onPageChange = { settingsPage = it },
                    buildInfo =
                        BuildInfo(
                            BuildConfig.VERSION_NAME,
                            BuildConfig.GIT_COMMIT,
                            BuildConfig.GIT_COMMIT_TIME,
                        ),
                )
            }
        app.monitoring && role == Role.PARENT -> {
            val sheet = if (openProbe) ParentSheet.PROBE else null
            LaunchedEffect(openProbe) { if (openProbe) openProbe = false }
            MonitorBackHandler()
            ParentMonitorScreen(
                controller,
                onSettings = { overlay = Overlay.SETTINGS },
                initialSheet = sheet,
                onChosenOther = { listRequested = true },
            )
        }
        app.monitoring -> {
            MonitorBackHandler()
            BabyMonitorScreen(controller, onSettings = { overlay = Overlay.SETTINGS })
        }
        pairing != null ->
            if (role == Role.BABY) PairBabyScreen(controller) else PairParentScreen(controller)
        else ->
            HomeScreen(
                controller,
                onSettings = { overlay = Overlay.SETTINGS },
                autoScanned = autoScanned,
                onAutoScanned = { autoScanned = true },
                listRequested = listRequested,
                onListShown = { listRequested = false },
            )
    }
}

/**
 * Back on the monitor moves the app to the background and keeps monitoring (`ui-appearance` "Back
 * behaviour while monitoring"). Sheets and dialogs register their own handler later, which takes
 * precedence, so back closes them first.
 */
@Composable
private fun MonitorBackHandler() {
    val env = LocalUiEnv.current
    val context = LocalContext.current
    BackHandler {
        val away = env.moveToBackground
        if (away != null) away() else context.findActivity()?.moveTaskToBack(true)
    }
}

/**
 * While the parent monitors, every screen laid over the monitor (Settings, set-up checks) keeps the
 * status bar on top (`connection-status` "Parent ambient bar"). The bar takes the top and side
 * system insets, so the screen below only pads what is left.
 */
@Composable
internal fun WithStatusBar(controller: AppController, content: @Composable () -> Unit) {
    val app by controller.app.collectAsState()
    val parent by controller.parent.collectAsState()
    if (!(app.monitoring && app.role == Role.PARENT)) {
        content()
        return
    }
    Column(
        Modifier.fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal)
            )
    ) {
        StatusBar(
            parent.link,
            customPeerName(app.peerName),
            backgroundProtection = app.backgroundProtection,
        )
        Box(Modifier.weight(1f).fillMaxWidth()) { content() }
    }
}

/** S10 full-screen noise alert (shown over the lock screen by AlertActivity). */
@Composable
fun NoiseAlertScreen(controller: AppController, onClose: () -> Unit) {
    AlertTheme(controller) { NoiseAlertContent(controller, onClose) }
}

/** S11 full-screen connection alarm / baby-stopped notice (shown by AlertActivity). */
@Composable
fun LinkAlarmScreen(controller: AppController, onClose: () -> Unit) {
    AlertTheme(controller) { LinkAlarmContent(controller, onClose) }
}

@Composable
private fun AlertTheme(controller: AppController, content: @Composable () -> Unit) {
    // Back on a full-screen alert does nothing: only its buttons end it (`ui-appearance` "Back
    // behaviour while monitoring"), so a stray swipe cannot hide a running alarm.
    BackHandler {}
    val env = LocalUiEnv.current
    val systemReduce = systemReduceMotion()
    CompositionLocalProvider(
        LocalUiEnv provides env.copy(reduceMotion = env.reduceMotion || systemReduce)
    ) {
        // Alerts are dark red on dark in every theme (never a white flash, alpha-ux-flows §6.3).
        GuardedSleepTheme(Theme.DARK, forceDark = true, content = content)
    }
}
