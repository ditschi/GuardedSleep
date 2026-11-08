package org.guardedsleep.app.ui

import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.ListenMode
import org.guardedsleep.app.ui.model.MissedEventsUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.SilenceRequest
import org.guardedsleep.app.ui.model.SilenceStatus
import org.guardedsleep.app.ui.model.SoundEventUi
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.screens.ParentSheet
import org.junit.Test

class ParentMonitorShots : ShotBase() {
    private val quietSince = WALL - 14 * 60_000L
    private val base =
        ParentMonitorState(
            link = Connected,
            peer = PeerStatusUi(batteryPct = 82, charging = true, silence = SilenceStatus.SILENT),
            level = 0.04f,
            quietSinceWallMs = quietSince,
        )

    private fun shoot(
        name: String,
        state: ParentMonitorState,
        locale: String = "en",
        sheet: ParentSheet? = null,
        app: AppState? = null,
        settings: SettingsState = SettingsState(),
        fontScale: Float = 1f,
    ) {
        val c =
            FakeAppController(
                app =
                    app
                        ?: AppState(
                            role = Role.PARENT,
                            onboardingDone = true,
                            paired = true,
                            monitoring = true,
                            peerName = null,
                        ),
                parent = state,
                settings = settings,
            )
        shot("parent_monitor/$name", locale, fontScale = fontScale) {
            ParentMonitorScreen(c, onSettings = {}, initialSheet = sheet)
        }
    }

    @Test fun connectedQuiet() = shoot("connected_quiet", base)

    @Test
    fun discreetIndicator() =
        shoot(
            "discreet_quiet",
            base,
            settings = SettingsState(alertStyle = AlertStyle.DISCREET),
        )

    @Test
    fun connectedNoise() =
        shoot(
            "connected_noise",
            base.copy(
                level = 0.7f,
                noiseAlertActive = true,
                noiseSinceWallMs = WALL - 20_000,
                quietSinceWallMs = null,
            ),
        )

    @Test
    fun afterNoise() =
        shoot(
            "after_noise",
            base.copy(
                level = 0.05f,
                quietSinceWallMs = null,
                lastSound = SoundEventUi(WALL - 90_000, 45_000),
            ),
        )

    @Test
    fun noiseSnoozed() =
        shoot(
            "noise_snoozed",
            base.copy(
                level = 0.6f,
                noiseAlertActive = true,
                noiseSinceWallMs = WALL - 20_000,
                quietSinceWallMs = null,
                snoozedUntilWallMs = WALL + 7 * 60_000L,
            ),
        )

    @Test
    fun linkPhasesAndLevels() {
        shoot("link_connecting", ParentMonitorState(link = link(LinkPhase.CONNECTING)))
        for (lvl in 0..3) {
            val ago = longArrayOf(8_000, 35_000, 95_000, 200_000)[lvl]
            shoot(
                "link_reconnecting_l$lvl",
                ParentMonitorState(link = link(LinkPhase.RECONNECTING, lvl, ago), peer = base.peer),
            )
        }
        shoot(
            "link_peer_stopped",
            ParentMonitorState(link = link(LinkPhase.PEER_STOPPED, 0, 4_000), peer = base.peer),
        )
        shoot(
            "link_needs_pairing",
            ParentMonitorState(
                link =
                    link(LinkPhase.FAILED_NEEDS_USER, 0, 4_000, failureKey = "pairing_key_mismatch")
            ),
        )
        shoot(
            "link_update_required",
            ParentMonitorState(
                link = link(LinkPhase.FAILED_NEEDS_USER, 0, 4_000, failureKey = "update_required")
            ),
        )
        shoot("link_stopped", ParentMonitorState(link = link(LinkPhase.STOPPED)))
    }

    @Test
    fun noBackgroundService() {
        val off =
            AppState(
                role = Role.PARENT,
                onboardingDone = true,
                paired = true,
                monitoring = true,
                backgroundProtection = false,
            )
        shoot("no_background", base, app = off)
        shoot("no_background_de", base, locale = "de", app = off)
    }

    @Test
    fun stalePairingHint() {
        val absent =
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
        shoot("stale_pairing_hint", absent)
        shoot("stale_pairing_hint_de", absent, locale = "de")
    }

    @Test
    fun acknowledged() =
        shoot(
            "level2_acknowledged",
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 2, 95_000, ack = true),
                peer = base.peer,
            ),
        )

    @Test
    fun weakConnection() =
        shoot("weak_connection", base.copy(link = Connected.copy(quality = LinkQuality.POOR)))

    @Test fun restoredBanner() = shoot("restored", base.copy(restoredAfterMs = 83_000))

    @Test
    fun missedEvents() {
        shoot(
            "missed_some",
            base.copy(missed = MissedEventsUi(300_000, 3, true, endWallMs = WALL, loudest = 0.7f)),
        )
        shoot(
            "missed_one",
            base.copy(missed = MissedEventsUi(300_000, 1, true, endWallMs = WALL, loudest = 0.3f)),
        )
        shoot("missed_none", base.copy(missed = MissedEventsUi(300_000, 0, true, endWallMs = WALL)))
        shoot(
            "missed_unknown",
            base.copy(missed = MissedEventsUi(300_000, null, false, endWallMs = WALL)),
        )
    }

    @Test
    fun silenceChips() {
        for (s in SilenceStatus.values()) {
            shoot("silence_${s.name.lowercase()}", base.copy(peer = base.peer.copy(silence = s)))
        }
        shoot(
            "silence_web_baby",
            base.copy(peer = base.peer.copy(silence = SilenceStatus.UNKNOWN, isWebClient = true)),
        )
        shoot(
            "sheet_silence_web",
            base.copy(peer = base.peer.copy(silence = SilenceStatus.UNKNOWN, isWebClient = true)),
            sheet = ParentSheet.SILENCE,
        )
        shoot(
            "sheet_silence_may_ring",
            base.copy(peer = base.peer.copy(silence = SilenceStatus.MAY_RING)),
            sheet = ParentSheet.SILENCE,
        )
        shoot(
            "sheet_silence_failed",
            base.copy(
                peer = base.peer.copy(silence = SilenceStatus.MAY_RING),
                silenceRequest = SilenceRequest.FAILED,
            ),
            sheet = ParentSheet.SILENCE,
        )
        shoot(
            "sheet_silence_no_access",
            base.copy(peer = base.peer.copy(silence = SilenceStatus.NO_ACCESS)),
            sheet = ParentSheet.SILENCE,
        )
    }

    @Test
    fun readinessChips() {
        shoot("readiness_red", base.copy(readiness = listOf(ReadinessIssue.ALARM_VOLUME_ZERO)))
        shoot("readiness_amber", base.copy(readiness = listOf(ReadinessIssue.DND_BYPASS_MISSING)))
        shoot(
            "readiness_headphones",
            base.copy(readiness = listOf(ReadinessIssue.HEADPHONES_CONNECTED)),
        )
        shoot("readiness_never_tested", base.copy(readiness = listOf(ReadinessIssue.NEVER_TESTED)))
        shoot(
            "readiness_many_chips",
            base.copy(
                readiness =
                    listOf(
                        ReadinessIssue.NOTIFICATIONS_BLOCKED,
                        ReadinessIssue.BATTERY_OPTIMIZED,
                        ReadinessIssue.NEVER_TESTED,
                    ),
                ownBatteryPct = 14,
                snoozedUntilWallMs = WALL + 600_000L,
            ),
        )
        shoot(
            "baby_battery_low",
            base.copy(peer = base.peer.copy(batteryPct = 12, charging = false)),
        )
        shoot(
            "sheet_readiness",
            base.copy(
                readiness =
                    listOf(
                        ReadinessIssue.ALARM_VOLUME_ZERO,
                        ReadinessIssue.FULL_SCREEN_BLOCKED,
                        ReadinessIssue.BATTERY_OPTIMIZED,
                    )
            ),
            sheet = ParentSheet.READINESS,
        )
    }

    @Test
    fun listenStates() {
        shoot("listen_hold", base.copy(listen = ListenMode.HOLD))
        shoot("listen_locked", base.copy(listen = ListenMode.LOCKED))
    }

    @Test
    fun snoozeAndOthers() {
        shoot("snoozed", base.copy(snoozedUntilWallMs = WALL + 8 * 60_000L))
        shoot("sheet_snooze", base, sheet = ParentSheet.SNOOZE)
        shoot(
            "sheet_snooze_active",
            base.copy(snoozedUntilWallMs = WALL + 8 * 60_000L),
            sheet = ParentSheet.SNOOZE,
        )
        shoot("sheet_probe", base, sheet = ParentSheet.PROBE)
        shoot(
            "sheet_probe_offline",
            ParentMonitorState(link = link(LinkPhase.RECONNECTING, 1, 35_000)),
            sheet = ParentSheet.PROBE,
        )
        shoot(
            "sheet_help_reconnect",
            ParentMonitorState(link = link(LinkPhase.RECONNECTING, 1, 35_000)),
            sheet = ParentSheet.HELP,
        )
        shoot("sheet_more", base, sheet = ParentSheet.MORE)
        shoot("dialog_stop", base, sheet = ParentSheet.STOP)
        shoot("security_warning", base.copy(securityWarning = true))
        shoot(
            "coach_first_run",
            base,
            app =
                AppState(
                    role = Role.PARENT,
                    onboardingDone = true,
                    paired = true,
                    monitoring = true,
                    coachPending = true,
                ),
        )
    }

    @Test
    fun unitWarningsAndNames() {
        val web =
            base.copy(
                peer =
                    base.peer.copy(
                        silence = SilenceStatus.UNKNOWN,
                        isWebClient = true,
                        charging = false,
                        unitWarnings =
                            listOf(
                                "page_hidden",
                                "not_charging",
                                "mic_permission_lost",
                                "wake_lock_off",
                                "resumed_from_sleep",
                            ),
                    )
            )
        shoot("unit_warnings", web)
        shoot("unit_warnings_de", web, "de")
        val named =
            AppState(
                role = Role.PARENT,
                onboardingDone = true,
                paired = true,
                monitoring = true,
                peerName = "Lena's tablet",
            )
        shoot("custom_name", base, app = named)
        shoot(
            "custom_name_l2",
            ParentMonitorState(link = link(LinkPhase.RECONNECTING, 2, 95_000), peer = base.peer),
            app = named,
        )
        shoot(
            "stale_chips_l1",
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 1, 35_000),
                peer = base.peer.copy(unitWarnings = listOf("not_charging")),
            ),
        )
        shoot(
            "stale_chips_l1",
            ParentMonitorState(link = link(LinkPhase.RECONNECTING, 1, 35_000), peer = base.peer),
            "de",
        )
        shoot("baby_threshold_line", base.copy(level = 0.3f, babyThreshold = 0.35f))
        shoot(
            "ack_note_de",
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 2, 95_000, ack = true),
                peer = base.peer,
            ),
            "de",
        )
        shoot("font200", base, "de", fontScale = 2f)
        shoot(
            "font200_l2",
            ParentMonitorState(link = link(LinkPhase.RECONNECTING, 2, 95_000), peer = base.peer),
            "de",
            fontScale = 2f,
        )
    }

    @Test
    fun localesDe() {
        shoot("connected_quiet", base, "de")
        shoot(
            "reconnecting_l2",
            ParentMonitorState(link = link(LinkPhase.RECONNECTING, 2, 95_000), peer = base.peer),
            "de",
        )
        shoot(
            "connected_noise",
            base.copy(
                level = 0.7f,
                noiseAlertActive = true,
                noiseSinceWallMs = WALL - 20_000,
                quietSinceWallMs = null,
            ),
            "de",
        )
        shoot(
            "silence_screen_may_light",
            base.copy(
                peer = base.peer.copy(silence = SilenceStatus.SCREEN_MAY_LIGHT),
                readiness = listOf(ReadinessIssue.DND_BYPASS_MISSING),
            ),
            "de",
        )
        shoot(
            "missed_some",
            base.copy(missed = MissedEventsUi(300_000, 3, true, endWallMs = WALL, loudest = 0.7f)),
            "de",
        )
        shoot("sheet_snooze", base, "de", ParentSheet.SNOOZE)
    }

    @Test
    fun localesFrEsPseudo() {
        for (l in listOf("fr", "es", "en-rXA")) {
            shoot(
                "connected_quiet",
                base.copy(readiness = listOf(ReadinessIssue.DND_BYPASS_MISSING)),
                l,
            )
            shoot(
                "reconnecting_l1",
                ParentMonitorState(
                    link = link(LinkPhase.RECONNECTING, 1, 35_000),
                    peer = base.peer,
                ),
                l,
            )
        }
    }
}
