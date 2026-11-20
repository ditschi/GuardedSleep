package org.guardedsleep.app.ui

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.focus.FocusRequester
import org.guardedsleep.app.ui.common.LocalUiEnv
import org.guardedsleep.app.ui.common.RowState
import org.guardedsleep.app.ui.common.UiEnv
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.DeviceCheckReason
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.MailboxReach
import org.guardedsleep.app.ui.model.MissedEventsUi
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
import org.guardedsleep.app.ui.screens.SettingsPage
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
        babyConnections = listOf(ConnectionUi("1", null, isDefault = true, selected = true)),
    )
internal const val CODE =
    "guardedsleep://pair?v=1&room=0123456789abcdef&psk=ZmFrZS1wc2stZm9yLXNjcmVlbnNob3RzLW9ubHk&url=wss%3A%2F%2Fsignal.example.org%2Fsig&exp=1792462740000"

class BabyShots : ShotBase() {
    private val calm =
        BabyMonitorState(
            link = Connected,
            dndAccess = true,
            silenceApplied = true,
            micLevel = 0.05f,
            ownBatteryPct = 82,
            ownPlugged = true,
            chargerSeen = true,
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

    private fun kiosk(
        name: String,
        b: BabyMonitorState,
        env: UiEnv = TestEnv,
        app: AppState = BabyApp,
        locale: String = "en",
    ) = shot("baby_kiosk/$name", locale, Role.BABY) { KioskPreview(b, env, app) }

    @androidx.compose.runtime.Composable
    private fun KioskPreview(b: BabyMonitorState, env: UiEnv, app: AppState) =
        CompositionLocalProvider(LocalUiEnv provides env) {
            BabyKioskScreen(FakeAppController(app = app, baby = b.copy(kioskLocked = true)))
        }

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
        val named =
            BabyApp.copy(
                connections =
                    listOf(ConnectionUi("a", "Travel cot", isDefault = true, selected = true))
            )
        kiosk("calm", calm, app = named)
        kiosk("calm", calm, app = named, locale = "de")
        kiosk("waiting", calm.copy(link = link(LinkPhase.CONNECTING)), app = named)
        kiosk(
            "pulse_level2",
            calm.copy(link = link(LinkPhase.RECONNECTING, 2, 95_000)),
            app = named,
        )
        kiosk(
            "pulse_level3",
            calm.copy(link = link(LinkPhase.RECONNECTING, 3, 200_000)),
            app = named,
        )
        kiosk(
            "parent_stopped",
            calm.copy(link = link(LinkPhase.PEER_STOPPED, 0, 4_000)),
            app = named,
        )
        kiosk(
            "reconnecting_edge",
            calm.copy(link = link(LinkPhase.RECONNECTING, 1, 40_000)),
            app = named,
        )
        kiosk("parent_talking", calm.copy(parentTalking = true), app = named)
        kiosk(
            "needs_attention",
            calm.copy(ownBatteryPct = 12, ownPlugged = false),
            app = named,
        )
        kiosk(
            "setup_confirmation",
            calm.copy(parentName = "Anna's phone", firstContactElapsedMs = ELAPSED - 15_000),
            app = named,
        )
        kiosk(
            "setup_confirmation_no_name",
            calm.copy(parentName = "", firstContactElapsedMs = ELAPSED - 15_000),
            app = BabyApp,
        )
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
    fun deviceCheckWithReasons() {
        val states =
            mapOf(
                SetupRow.MIC to RowState.DONE,
                SetupRow.BACKGROUND to RowState.DONE,
                SetupRow.SILENCE to RowState.DONE,
            )
        shot("setup/baby_check_reasons", "en", Role.BABY) {
            SetupContent(
                Role.BABY,
                SetupChecks(states),
                DeviceCheck.NOT_RECOMMENDED,
                {},
                {},
                deviceCheckReasons =
                    listOf(
                        DeviceCheckReason.BATTERY_LOW_NOT_CHARGING,
                        DeviceCheckReason.NO_MAILBOX_REACHABLE,
                    ),
                deviceCheckMailboxes = listOf(MailboxReach("gs.mydata.rocks", false)),
                onDeviceCheckAgain = {},
            )
        }
    }

    @Test
    fun deviceCheckWithOneReachableMailbox() {
        val states =
            mapOf(
                SetupRow.MIC to RowState.DONE,
                SetupRow.BACKGROUND to RowState.DONE,
                SetupRow.SILENCE to RowState.DONE,
            )
        shot("setup/baby_check_partial", "en", Role.BABY) {
            SetupContent(
                Role.BABY,
                SetupChecks(states),
                DeviceCheck.LIMITED,
                {},
                {},
                deviceCheckReasons =
                    listOf(
                        DeviceCheckReason.SOME_MAILBOXES_UNREACHABLE,
                        DeviceCheckReason.ROOM_NEAR_THRESHOLD,
                    ),
                deviceCheckMailboxes =
                    listOf(
                        MailboxReach("gs.mydata.rocks", true),
                        MailboxReach("own.example", false),
                    ),
                onDeviceCheckAgain = {},
            )
        }
    }

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
        // Frame 01: a returning user with two parent connections and one baby connection.
        val returning =
            AppState(
                role = null,
                lastRole = Role.PARENT,
                onboardingDone = true,
                paired = true,
                peerName = "Nursery",
                parentConnections =
                    listOf(
                        ConnectionUi("1", "Nursery", isDefault = false, selected = true),
                        ConnectionUi("2", "Grandma's flat", isDefault = false, selected = false),
                    ),
                babyConnections = listOf(ConnectionUi("1", "Travel cot", false, true)),
                canAddParentPairing = false,
            )
        shot("home/returning") { HomeScreen(FakeAppController(app = returning), {}) }
        shot("home/returning", "de") { HomeScreen(FakeAppController(app = returning), {}) }
        shot("home/returning_day", theme = Theme.LIGHT) {
            HomeScreen(FakeAppController(app = returning), {})
        }
        shot("home/returning_font2", fontScale = 2f) {
            HomeScreen(FakeAppController(app = returning), {})
        }
        shot("home/returning_vibrate_only") {
            HomeScreen(
                FakeAppController(
                    app = returning,
                    settings = SettingsState(alertStyle = AlertStyle.DISCREET),
                ),
                {},
            )
        }
        // One role only: the other role's header offers "Set up new devices".
        shot("home/parent_only") {
            HomeScreen(
                FakeAppController(
                    app =
                        returning.copy(
                            babyConnections = emptyList(),
                            canAddParentPairing = true,
                            defaultRole = Role.PARENT,
                        )
                ),
                {},
            )
        }
        // Nothing saved yet (the first-start choice was made, the pairing cancelled).
        shot("home/empty") {
            HomeScreen(
                FakeAppController(
                    app =
                        returning.copy(
                            role = Role.PARENT,
                            paired = false,
                            parentConnections = emptyList(),
                            babyConnections = emptyList(),
                            canAddParentPairing = true,
                        )
                ),
                {},
                autoScanned = true,
            )
        }
    }

    private fun twoConnections(app: AppState) =
        app.copy(
            peerName = "Home – old phone",
            parentConnections =
                listOf(
                    ConnectionUi("1", "Home – old phone", isDefault = true, selected = true),
                    ConnectionUi("2", null, isDefault = false, selected = false),
                ),
            connections =
                listOf(
                    ConnectionUi("1", "Home – old phone", isDefault = true, selected = true),
                    ConnectionUi("2", null, isDefault = false, selected = false),
                ),
            canAddParentPairing = false,
        )

    @Test
    fun settings() {
        val parentApp =
            AppState(
                role = Role.PARENT,
                onboardingDone = true,
                paired = true,
                peerName = "Lena",
                parentConnections =
                    listOf(ConnectionUi("1", "Lena", isDefault = false, selected = true)),
            )
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
                initialPage = SettingsPage.ADVANCED,
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
        shot("settings/parent_two_connections") {
            SettingsScreen(FakeAppController(app = twoConnections(parentApp)), {}, {}, {})
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
                initialPage = SettingsPage.ADVANCED,
            )
        }
        shot("settings/parent_alerts") {
            SettingsScreen(
                FakeAppController(app = parentApp),
                {},
                {},
                {},
                initialPage = SettingsPage.ALERTS,
            )
        }
        shot("settings/parent_open_at_start") {
            SettingsScreen(
                FakeAppController(app = twoConnections(parentApp)),
                {},
                {},
                {},
                initialPage = SettingsPage.OPEN_AT_START,
            )
        }
        shot("settings/parent_connections") {
            SettingsScreen(
                FakeAppController(app = twoConnections(parentApp)),
                {},
                {},
                {},
                initialPage = SettingsPage.CONNECTIONS,
            )
        }
        shot("settings/parent_monitoring") {
            SettingsScreen(
                FakeAppController(app = twoConnections(parentApp).copy(monitoring = true)),
                {},
                {},
                {},
            )
        }
        shot("settings/parent_basic_day", theme = Theme.LIGHT) {
            SettingsScreen(FakeAppController(app = twoConnections(parentApp)), {}, {}, {})
        }
        shot("settings/parent_monitoring_day", theme = Theme.LIGHT) {
            SettingsScreen(
                FakeAppController(app = twoConnections(parentApp).copy(monitoring = true)),
                {},
                {},
                {},
            )
        }
        shot("settings/parent_basic_font2", fontScale = 2f) {
            SettingsScreen(FakeAppController(app = parentApp), {}, {}, {})
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
        noise("test", base.copy(noiseIsTest = true, noiseAlertActive = false))
        shot("alert/test_discreet", "en") {
            NoiseAlertScreen(
                FakeAppController(
                    parent = base.copy(noiseIsTest = true, noiseAlertActive = false),
                    settings = SettingsState(alertStyle = AlertStyle.DISCREET),
                ),
                {},
            )
        }
        noise("ended", base.copy(noiseAlertActive = false, level = 0.02f))
        shot("alert/escalated", "en") {
            NoiseAlertScreen(
                FakeAppController(
                    parent = base.copy(noiseEscalatedAfterMin = 2),
                    settings = SettingsState(alertStyle = AlertStyle.DISCREET),
                ),
                {},
            )
        }
        shot("alert/discreet", "en") {
            NoiseAlertScreen(
                FakeAppController(
                    parent = base,
                    settings = SettingsState(alertStyle = AlertStyle.DISCREET),
                ),
                {},
            )
        }
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
            ParentMonitorState(
                link = Connected,
                peer = PeerStatusUi(batteryPct = 80),
                restoredAfterMs = 108_000,
                missed = MissedEventsUi(108_000, 0, complete = true),
            ),
        )
        // Frame 25: a first connection that never answered, from one minute.
        alarm(
            "first_connection_level2",
            ParentMonitorState(
                link =
                    link(LinkPhase.CONNECTING, 2, 70_000, cause = CauseHint.PEER_ABSENT)
                        .copy(lastAliveElapsedMs = null),
                peer = base.peer,
            ),
        )
        // Frame 46: the same alert at font scale 2.0, the action pinned and fully visible.
        shot("alarm/level2_fontscale2", "en", fontScale = 2.0f) {
            LinkAlarmScreen(
                FakeAppController(
                    parent =
                        ParentMonitorState(
                            link =
                                link(
                                    LinkPhase.RECONNECTING,
                                    2,
                                    72_000,
                                    cause = CauseHint.PEER_ABSENT,
                                ),
                            peer = base.peer,
                        )
                ),
                {},
            )
        }
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

/** The status bar of every stage, in Dusk and Day (frames 03, 09 to 11, 29). */
class StatusBarShots : ShotBase() {
    private fun bar(
        name: String,
        l: org.guardedsleep.app.ui.model.LinkUi,
        theme: Theme = Theme.DARK,
    ) =
        shot("status_bar/$name", theme = theme) {
            org.guardedsleep.app.ui.common.StatusBar(l, "Nursery")
        }

    @Test
    fun stages() {
        bar("connected", Connected)
        bar("connecting", link(LinkPhase.CONNECTING, 0, 5_000).copy(lastAliveElapsedMs = null))
        bar("reconnecting", link(LinkPhase.RECONNECTING, 0, 10_000))
        bar(
            "not_connected_yet",
            link(LinkPhase.CONNECTING, 1, 25_000).copy(lastAliveElapsedMs = null),
        )
        bar("cant_reach", link(LinkPhase.RECONNECTING, 1, 25_000))
        bar("no_connection", link(LinkPhase.RECONNECTING, 2, 72_000))
        bar("stopped", link(LinkPhase.PEER_STOPPED, 0, 3_000))
        bar("connected_day", Connected, Theme.LIGHT)
        bar("cant_reach_day", link(LinkPhase.RECONNECTING, 1, 25_000), Theme.LIGHT)
    }
}
