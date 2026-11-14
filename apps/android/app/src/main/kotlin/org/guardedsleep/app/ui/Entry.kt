package org.guardedsleep.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import org.guardedsleep.app.BuildConfig
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.systemReduceMotion
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.Theme
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
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.screens.SetupScreen
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
        GuardedSleepTheme(settings.theme, forceDark = app.role == Role.BABY) {
            AppNavigator(controller)
        }
    }
}

private enum class Overlay {
    NONE,
    SETTINGS,
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
            SetupScreen(
                controller,
                role,
                onboarding = false,
                onBack = { overlay = Overlay.SETTINGS },
            )
        overlay == Overlay.SETTINGS ->
            SettingsScreen(
                controller,
                onBack = { overlay = Overlay.NONE },
                onOpenSetup = { overlay = Overlay.SETUP },
                onTestAlert = {
                    openProbe = true
                    overlay = Overlay.NONE
                },
                buildInfo =
                    BuildInfo(
                        BuildConfig.VERSION_NAME,
                        BuildConfig.GIT_COMMIT,
                        BuildConfig.GIT_COMMIT_TIME,
                    ),
            )
        app.monitoring && role == Role.PARENT -> {
            val sheet = if (openProbe) ParentSheet.PROBE else null
            LaunchedEffect(openProbe) { if (openProbe) openProbe = false }
            ParentMonitorScreen(
                controller,
                onSettings = { overlay = Overlay.SETTINGS },
                initialSheet = sheet,
                onChosenOther = { listRequested = true },
            )
        }
        app.monitoring -> BabyMonitorScreen(controller, onSettings = { overlay = Overlay.SETTINGS })
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
    val env = LocalUiEnv.current
    val systemReduce = systemReduceMotion()
    CompositionLocalProvider(
        LocalUiEnv provides env.copy(reduceMotion = env.reduceMotion || systemReduce)
    ) {
        // Alerts are dark red on dark in every theme (never a white flash, alpha-ux-flows §6.3).
        GuardedSleepTheme(Theme.DARK, forceDark = true, content = content)
    }
}
