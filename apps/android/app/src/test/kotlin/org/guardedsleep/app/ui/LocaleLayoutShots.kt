package org.guardedsleep.app.ui

import androidx.compose.runtime.remember
import androidx.compose.ui.focus.FocusRequester
import org.guardedsleep.app.ui.model.AppState
import org.guardedsleep.app.ui.model.BabyMonitorState
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.PairingPhase
import org.guardedsleep.app.ui.model.PairingState
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.screens.BabyMonitorScreen
import org.guardedsleep.app.ui.screens.PairBabyScreen
import org.guardedsleep.app.ui.screens.ParentMonitorScreen
import org.guardedsleep.app.ui.screens.ParentPairContent
import org.guardedsleep.app.ui.screens.RoleScreen
import org.guardedsleep.app.ui.screens.SettingsScreen
import org.junit.Test

/**
 * Layout check of the main screens in the right-to-left locale (ar) and in the platform
 * pseudo-locales (en-XA accented and long, ar-XB mirrored). German already has a shot per screen in
 * the per-screen classes (`*_de.png`); this class adds the same eight screens for Arabic and the
 * pseudo-locales, so a mirrored layout or clipped text shows up as a baseline diff (expand-locales
 * task 5.1).
 */
class LocaleLayoutShots : ShotBase() {
    private val babyApp =
        AppState(
            role = Role.BABY,
            onboardingDone = true,
            paired = true,
            monitoring = true,
            peerName = null,
        )
    private val parentApp =
        AppState(
            role = Role.PARENT,
            onboardingDone = true,
            paired = true,
            monitoring = true,
            peerName = "Lena",
        )
    private val calm =
        BabyMonitorState(
            link = Connected,
            dndAccess = true,
            silenceApplied = true,
            micLevel = 0.05f,
        )
    private val loud =
        ParentMonitorState(
            link = Connected,
            peer = PeerStatusUi(batteryPct = 80, charging = true),
            level = 0.75f,
            noiseAlertActive = true,
            noiseSinceWallMs = WALL - 12_000,
        )
    private val lost =
        ParentMonitorState(
            link = link(LinkPhase.RECONNECTING, 2, 95_000, cause = CauseHint.PEER_ABSENT),
            peer = PeerStatusUi(batteryPct = 80, charging = true),
        )
    private val quiet =
        ParentMonitorState(
            link = Connected,
            peer = PeerStatusUi(batteryPct = 82, charging = true),
            level = 0.04f,
            quietSinceWallMs = WALL - 14 * 60_000L,
        )

    private fun screens(locale: String) {
        shot("locale_layout/role", locale, null) { RoleScreen(FakeAppController(app = AppState())) }
        shot("locale_layout/pair_baby", locale, Role.BABY) {
            PairBabyScreen(
                FakeAppController(
                    app = babyApp.copy(monitoring = false),
                    pairing = PairingState(PairingPhase.SHOWING_CODE, CODE, ELAPSED + 299_000L),
                )
            )
        }
        shot("locale_layout/pair_parent", locale) {
            val focus = remember { FocusRequester() }
            ParentPairContent(PairingPhase.SCANNING, null, true, "", {}, focus, {}, {}, {}, {}, {})
        }
        shot("locale_layout/baby_monitor", locale, Role.BABY) {
            BabyMonitorScreen(FakeAppController(app = babyApp, baby = calm), onSettings = {})
        }
        shot("locale_layout/parent_monitor", locale) {
            ParentMonitorScreen(
                FakeAppController(app = parentApp, parent = quiet),
                onSettings = {},
                initialSheet = null,
            )
        }
        shot("locale_layout/noise_alert", locale) {
            NoiseAlertScreen(FakeAppController(parent = loud), {})
        }
        shot("locale_layout/link_alarm", locale) {
            LinkAlarmScreen(FakeAppController(parent = lost), {})
        }
        shot("locale_layout/settings", locale) {
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

    @Test fun arabic() = screens("ar")

    @Test fun arabicPseudoRtl() = screens("ar-rXB")

    @Test fun englishPseudoAccented() = screens("en-rXA")
}
