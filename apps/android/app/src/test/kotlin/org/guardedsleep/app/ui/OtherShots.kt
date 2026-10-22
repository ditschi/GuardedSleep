package org.guardedsleep.app.ui

import androidx.compose.runtime.remember
import androidx.compose.ui.focus.FocusRequester
import org.guardedsleep.app.ui.common.RowState
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.PairingError
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.Theme
import org.guardedsleep.app.ui.screens.BabyKioskScreen
import org.guardedsleep.app.ui.screens.BabyMonitorScreen
import org.guardedsleep.app.ui.screens.HomeScreen
import org.guardedsleep.app.ui.screens.PairBabyScreen
import org.guardedsleep.app.ui.screens.ParentPairContent
import org.guardedsleep.app.ui.screens.RoleScreen
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.guardedsleep.app.ui.screens.SetupChecks
import org.guardedsleep.app.ui.screens.SetupContent
import org.guardedsleep.app.ui.screens.SetupRow
import org.junit.Test

private val BabyApp =
    AppState(
        role = Role.BABY,
        onboardingDone = true,
        paired = true,
        monitoring = true,
        peerName = null,
    )
private const val CODE =
    "guardedsleep://pair?v=1&room=0123456789abcdef&psk=ZmFrZS1wc2stZm9yLXNjcmVlbnNob3RzLW9ubHk&url=wss%3A%2F%2Fsignal.example.org%2Fsig&exp=1792462740000"

class BabyShots : ShotBase() {
    private val calm =
        BabyMonitorState(
            link = Connected,
            dndAccess = true,
            silenceApplied = true,
            micLevel = 0.05f,
        )

    private fun monitor(name: String, b: BabyMonitorState, locale: String = "en") =
        shot("baby_monitor/$name", locale, Role.BABY) {
            BabyMonitorScreen(FakeAppController(app = BabyApp, baby = b), onSettings = {})
        }

    @Test
    fun noBackgroundService() =
        shot("baby_monitor/no_background", "en", Role.BABY) {
            BabyMonitorScreen(
                FakeAppController(app = BabyApp.copy(backgroundProtection = false), baby = calm),
                onSettings = {},
            )
        }

    private fun kiosk(name: String, b: BabyMonitorState) =
        shot("baby_kiosk/$name", "en", Role.BABY) { KioskPreview(b) }

    @androidx.compose.runtime.Composable
    private fun KioskPreview(b: BabyMonitorState) =
        BabyKioskScreen(FakeAppController(app = BabyApp, baby = b.copy(kioskLocked = true)))

    @Test
    fun monitorStates() {
        monitor("connected", calm)
        monitor("waiting", calm.copy(link = link(LinkPhase.CONNECTING)))
        monitor("reconnecting", calm.copy(link = link(LinkPhase.RECONNECTING, 0, 8_000)))
        monitor("not_responding", calm.copy(link = link(LinkPhase.RECONNECTING, 1, 40_000)))
        monitor("parent_stopped", calm.copy(link = link(LinkPhase.PEER_STOPPED, 0, 4_000)))
        monitor("needs_pairing", calm.copy(link = link(LinkPhase.FAILED_NEEDS_USER)))
        monitor("silence_restored_notice", calm.copy(silenceRestoredNotice = true))
        monitor(
            "no_dnd_access",
            calm.copy(dndAccess = false, silenceApplied = false, micSilent = true),
        )
        monitor("connected", calm, "de")
    }

    @Test
    fun kioskStates() {
        kiosk("calm", calm)
        kiosk("pulse_level2", calm.copy(link = link(LinkPhase.RECONNECTING, 2, 95_000)))
        kiosk("pulse_level3", calm.copy(link = link(LinkPhase.RECONNECTING, 3, 200_000)))
        kiosk("parent_stopped", calm.copy(link = link(LinkPhase.PEER_STOPPED, 0, 4_000)))
        kiosk("reconnecting_edge", calm.copy(link = link(LinkPhase.RECONNECTING, 1, 40_000)))
    }
}

class PairShots : ShotBase() {
    private fun baby(name: String, p: PairingState?, locale: String = "en") =
        shot("pair_baby/$name", locale, Role.BABY) {
            PairBabyScreen(FakeAppController(app = BabyApp.copy(monitoring = false), pairing = p))
        }

    @Test
    fun babyCode() {
        val exp = ELAPSED + 4 * 60_000L + 59_000L
        baby("code", PairingState(PairingPhase.SHOWING_CODE, CODE, exp))
        baby("code", PairingState(PairingPhase.SHOWING_CODE, CODE, exp), "de")
        baby("expired", PairingState(PairingPhase.EXPIRED, CODE, ELAPSED - 1_000))
        baby("parent_found", PairingState(PairingPhase.CONNECTING, CODE, exp))
        baby("paired", PairingState(PairingPhase.PAIRED, CODE, exp))
        baby("failed", PairingState(PairingPhase.FAILED, CODE, exp))
    }

    private fun parent(
        name: String,
        phase: PairingPhase,
        err: PairingError?,
        granted: Boolean = true,
        locale: String = "en",
    ) =
        shot("pair_parent/$name", locale) {
            val focus = remember { FocusRequester() }
            ParentPairContent(phase, err, granted, "", {}, focus, {}, {}, {}, {}, {})
        }

    @Test
    fun parentStates() {
        parent("scanning", PairingPhase.SCANNING, null)
        parent("scanning", PairingPhase.SCANNING, null, locale = "de")
        parent("connecting", PairingPhase.CONNECTING, null)
        parent("camera_denied", PairingPhase.SCANNING, PairingError.CAMERA_DENIED, granted = false)
        for (e in PairingError.values().filter { it != PairingError.CAMERA_DENIED }) {
            parent("error_${e.name.lowercase()}", PairingPhase.FAILED, e)
        }
        parent("error_unreachable", PairingPhase.FAILED, PairingError.UNREACHABLE, locale = "de")
    }
}

class SetupHomeSettingsShots : ShotBase() {
    private fun setup(
        name: String,
        role: Role,
        states: Map<SetupRow, RowState>,
        check: DeviceCheck = DeviceCheck.OK,
        locale: String = "en",
    ) = shot("setup/$name", locale, role) { SetupContent(role, SetupChecks(states), check, {}, {}) }

    @Test
    fun roleScreen() {
        shot("role/role", "en", null) { RoleScreen(FakeAppController(app = AppState())) }
        shot("role/role", "de", null) { RoleScreen(FakeAppController(app = AppState())) }
    }

    @Test
    fun setupScreens() {
        setup(
            "baby_todo",
            Role.BABY,
            mapOf(
                SetupRow.MIC to RowState.TODO,
                SetupRow.BACKGROUND to RowState.TODO,
                SetupRow.SILENCE to RowState.TODO,
            ),
            DeviceCheck.UNKNOWN,
        )
        setup(
            "baby_done",
            Role.BABY,
            mapOf(
                SetupRow.MIC to RowState.DONE,
                SetupRow.BACKGROUND to RowState.DONE,
                SetupRow.SILENCE to RowState.DONE,
            ),
        )
        setup(
            "baby_mic_blocked",
            Role.BABY,
            mapOf(
                SetupRow.MIC to RowState.BLOCKED,
                SetupRow.BACKGROUND to RowState.DONE,
                SetupRow.SILENCE to RowState.TODO,
            ),
            DeviceCheck.LIMITED,
        )
        setup(
            "parent_todo",
            Role.PARENT,
            mapOf(
                SetupRow.NOTIFICATIONS to RowState.TODO,
                SetupRow.LOCK_SCREEN to RowState.TODO,
                SetupRow.DND_BYPASS to RowState.TODO,
                SetupRow.BACKGROUND to RowState.TODO,
            ),
        )
        setup("parent_done", Role.PARENT, SetupRow.values().associateWith { RowState.DONE })
        setup(
            "parent_todo",
            Role.PARENT,
            mapOf(SetupRow.NOTIFICATIONS to RowState.BLOCKED),
            locale = "de",
        )
    }

    @Test
    fun home() {
        val saved =
            AppState(
                role = Role.PARENT,
                onboardingDone = true,
                paired = true,
                peerName = "Lena",
                lastUsedWallMs = WALL - 8 * 3_600_000L,
            )
        shot("home/parent_saved") { HomeScreen(FakeAppController(app = saved), {}) }
        shot("home/baby_saved", "en", Role.BABY) {
            HomeScreen(FakeAppController(app = saved.copy(role = Role.BABY, peerName = null)), {})
        }
        shot("home/empty") { HomeScreen(FakeAppController(app = saved.copy(paired = false)), {}) }
        shot("home/parent_saved", "de") { HomeScreen(FakeAppController(app = saved), {}) }
    }

    @Test
    fun settings() {
        val parentApp =
            AppState(role = Role.PARENT, onboardingDone = true, paired = true, peerName = "Lena")
        shot("settings/parent_basic") {
            SettingsScreen(
                FakeAppController(
                    app = parentApp,
                    settings = SettingsState(lastTestWallMs = WALL - 86_400_000L),
                ),
                {},
                {},
                {},
            )
        }
        shot("settings/parent_advanced") {
            SettingsScreen(
                FakeAppController(
                    app = parentApp,
                    settings =
                        SettingsState(
                            advancedEnabled = true,
                            signalingUrls =
                                listOf(
                                    "wss://signal.example.org/sig",
                                    "wss://backup.example.net/sig",
                                ),
                            turnUrl = "turn:relay.example.org:3478",
                            alertOutput = AlertOutput.SPEAKER_ONLY,
                            theme = Theme.LIGHT,
                        ),
                ),
                {},
                {},
                {},
            )
        }
        shot("settings/baby_basic", "en", Role.BABY) {
            SettingsScreen(
                FakeAppController(
                    app = BabyApp,
                    settings = SettingsState(),
                    baby = BabyMonitorState(link = Connected, dndAccess = true, micLevel = 0.2f),
                ),
                {},
                {},
                {},
            )
        }
        shot("settings/parent_basic", "de") {
            SettingsScreen(FakeAppController(app = parentApp), {}, {}, {})
        }
        shot("settings/parent_advanced", "de") {
            SettingsScreen(
                FakeAppController(
                    app = parentApp,
                    settings = SettingsState(advancedEnabled = true),
                ),
                {},
                {},
                {},
            )
        }
    }
}

class AlertShots : ShotBase() {
    private val base =
        ParentMonitorState(
            link = Connected,
            peer = PeerStatusUi(batteryPct = 80, charging = true),
            level = 0.75f,
            noiseAlertActive = true,
            noiseSinceWallMs = WALL - 12_000,
        )

    private fun noise(name: String, p: ParentMonitorState, locale: String = "en") =
        shot("alert/$name", locale) { NoiseAlertScreen(FakeAppController(parent = p), {}) }

    private fun alarm(name: String, p: ParentMonitorState, locale: String = "en") =
        shot("alarm/$name", locale) { LinkAlarmScreen(FakeAppController(parent = p), {}) }

    @Test
    fun noBackgroundAlarm() {
        shot("alarm/no_background", "en") {
            LinkAlarmScreen(
                FakeAppController(
                    app =
                        AppState(
                            role = Role.PARENT,
                            onboardingDone = true,
                            paired = true,
                            monitoring = true,
                            backgroundProtection = false,
                        ),
                    parent = base,
                ),
                {},
            )
        }
        shot("alarm/stale_pairing_hint", "de") {
            LinkAlarmScreen(
                FakeAppController(
                    parent =
                        ParentMonitorState(
                            link =
                                link(
                                        LinkPhase.RECONNECTING,
                                        2,
                                        700_000,
                                        cause = org.guardedsleep.app.ui.model.CauseHint.PEER_ABSENT,
                                    )
                                    .copy(stalePairingHint = true)
                        )
                ),
                {},
            )
        }
    }

    @Test
    fun noise() {
        noise("sounding", base)
        noise("test", base.copy(noiseIsTest = true))
        noise("ended", base.copy(noiseAlertActive = false, level = 0.02f))
        noise("sounding", base, "de")
    }

    @Test
    fun linkAlarms() {
        alarm(
            "level2",
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 2, 95_000, cause = CauseHint.PEER_ABSENT),
                peer = base.peer,
            ),
        )
        alarm(
            "level3",
            ParentMonitorState(
                link =
                    link(LinkPhase.RECONNECTING, 3, 200_000, cause = CauseHint.SERVER_UNREACHABLE),
                peer = base.peer,
            ),
        )
        alarm(
            "level2_acknowledged",
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 2, 95_000, ack = true),
                peer = base.peer,
            ),
        )
        alarm("peer_stopped", ParentMonitorState(link = link(LinkPhase.PEER_STOPPED, 0, 3_000)))
        alarm(
            "battery_critical",
            ParentMonitorState(
                link = Connected,
                peer = PeerStatusUi(batteryPct = 4, charging = false),
            ),
        )
        alarm(
            "restored",
            ParentMonitorState(link = Connected, peer = PeerStatusUi(batteryPct = 80)),
        )
        alarm("needs_pairing", ParentMonitorState(link = link(LinkPhase.FAILED_NEEDS_USER)))
        alarm(
            "level2",
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 2, 95_000, cause = CauseHint.PEER_ABSENT)
            ),
            "de",
        )
        alarm(
            "peer_stopped",
            ParentMonitorState(link = link(LinkPhase.PEER_STOPPED, 0, 3_000)),
            "de",
        )
    }
}
