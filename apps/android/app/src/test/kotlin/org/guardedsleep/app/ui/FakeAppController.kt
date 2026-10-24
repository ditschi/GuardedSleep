package org.guardedsleep.app.ui

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.TurnSaveResult

/**
 * Test double: plain state flows plus a call log. Listen and kiosk changes are applied to state
 * like the real controller would.
 */
class FakeAppController(
    app: AppState =
        AppState(role = Role.PARENT, onboardingDone = true, paired = true, peerName = null),
    settings: SettingsState = SettingsState(),
    parent: ParentMonitorState = ParentMonitorState(),
    baby: BabyMonitorState = BabyMonitorState(),
    pairing: PairingState? = null,
) : AppController {
    val appFlow = MutableStateFlow(app)
    val settingsFlow = MutableStateFlow(settings)
    val parentFlow = MutableStateFlow(parent)
    val babyFlow = MutableStateFlow(baby)
    val pairingFlow = MutableStateFlow(pairing)
    val calls = mutableListOf<String>()

    override val app: StateFlow<AppState>
        get() = appFlow

    override val settings: StateFlow<SettingsState>
        get() = settingsFlow

    override val parent: StateFlow<ParentMonitorState>
        get() = parentFlow

    override val baby: StateFlow<BabyMonitorState>
        get() = babyFlow

    override val pairing: StateFlow<PairingState?>
        get() = pairingFlow

    override fun chooseRole(role: Role) {
        calls += "chooseRole:$role"
        appFlow.value = appFlow.value.copy(role = role)
    }

    override fun completeOnboarding() {
        calls += "completeOnboarding"
        appFlow.value = appFlow.value.copy(onboardingDone = true)
    }

    override fun startPairing() {
        calls += "startPairing"
    }

    override fun cancelPairing() {
        calls += "cancelPairing"
        pairingFlow.value = null
    }

    override fun submitPairingCode(payload: String) {
        calls += "submitPairingCode"
    }

    override fun refreshPairingCode() {
        calls += "refreshPairingCode"
    }

    override fun startMonitoring() {
        calls += "startMonitoring"
        appFlow.value = appFlow.value.copy(monitoring = true)
    }

    override fun stopMonitoring() {
        calls += "stopMonitoring"
        appFlow.value = appFlow.value.copy(monitoring = false)
    }

    override fun forgetPairing() {
        calls += "forgetPairing"
    }

    override fun updateSettings(transform: (SettingsState) -> SettingsState) {
        calls += "updateSettings"
        settingsFlow.value = transform(settingsFlow.value)
    }

    override fun setTurnServer(url: String, username: String, password: String): TurnSaveResult {
        // Never record the password.
        calls += "setTurnServer:$url:$username:${password.isNotEmpty()}"
        val u = url.trim()
        if (u.isEmpty()) {
            clearTurnServer()
            return TurnSaveResult.CLEARED
        }
        if (!Regex("^turns?:\\S+$").matches(u)) return TurnSaveResult.INVALID_ADDRESS
        if (username.isEmpty() != password.isEmpty() && !settingsFlow.value.turnPasswordSaved) {
            return TurnSaveResult.INVALID_CREDENTIALS
        }
        settingsFlow.value =
            settingsFlow.value.copy(
                turnUrl = u,
                turnUsername = username.ifEmpty { null },
                turnPasswordSaved = password.isNotEmpty() || settingsFlow.value.turnPasswordSaved,
            )
        return TurnSaveResult.SAVED
    }

    override fun clearTurnServer() {
        calls += "clearTurnServer"
        settingsFlow.value =
            settingsFlow.value.copy(turnUrl = null, turnUsername = null, turnPasswordSaved = false)
    }

    override fun setListen(mode: ListenMode) {
        calls += "setListen:$mode"
        parentFlow.value = parentFlow.value.copy(listen = mode)
    }

    override fun requestTestAlert() {
        calls += "requestTestAlert"
    }

    override fun snooze(minutes: Int) {
        calls += "snooze:$minutes"
    }

    override fun cancelSnooze() {
        calls += "cancelSnooze"
    }

    override fun acknowledgeAlarm() {
        calls += "acknowledgeAlarm"
    }

    override fun stopAlertSound() {
        calls += "stopAlertSound"
    }

    override fun requestSilenceBaby() {
        calls += "requestSilenceBaby"
    }

    override fun tryReconnectNow() {
        calls += "tryReconnectNow"
    }

    override fun dismissMissedEvents() {
        calls += "dismissMissedEvents"
    }

    override fun setKioskLocked(locked: Boolean) {
        calls += "setKioskLocked:$locked"
        babyFlow.value = babyFlow.value.copy(kioskLocked = locked)
    }

    override fun dismissSilenceRestoredNotice() {
        calls += "dismissSilenceRestoredNotice"
    }

    override fun reportTestResult(heardAndFelt: Boolean) {
        calls += "reportTestResult:$heardAndFelt"
    }

    override fun dismissCoach() {
        calls += "dismissCoach"
    }
}
