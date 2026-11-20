package org.guardedsleep.app.ui

import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.BabyBattery
import org.guardedsleep.app.ui.model.Banner
import org.guardedsleep.app.ui.model.BannerKind
import org.guardedsleep.app.ui.model.BannerLevel
import org.guardedsleep.app.ui.model.LevelHistory
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.LinkUi
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.ReadinessIssue
import org.guardedsleep.app.ui.model.SilenceStatus
import org.guardedsleep.app.ui.model.bannerLevel
import org.guardedsleep.app.ui.model.parentBanners
import org.guardedsleep.app.ui.model.rankBanners
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The same table the web client runs in `banners.test.ts` (`parent-alerts` "Banner priority on the
 * monitor"); keep the two in step.
 */
class BannersTest {
    private fun b(kind: BannerKind, id: String = kind.name.lowercase()) = Banner(kind, id)

    private fun kinds(l: List<Banner>) = l.map { it.kind }

    @Test
    fun fourNoticesAtOnce() {
        val s =
            rankBanners(
                listOf(
                    b(BannerKind.MIC_SILENT),
                    b(BannerKind.READINESS),
                    b(BannerKind.BABY_BATTERY_LOW),
                    b(BannerKind.VIBRATE_ONLY),
                )
            )
        assertEquals(listOf(BannerKind.MIC_SILENT, BannerKind.READINESS), kinds(s.shown))
        assertEquals(listOf(BannerKind.BABY_BATTERY_LOW, BannerKind.VIBRATE_ONLY), kinds(s.folded))
    }

    @Test
    fun theInputOrderDoesNotMatter() {
        val s =
            rankBanners(
                listOf(
                    b(BannerKind.VIBRATE_ONLY),
                    b(BannerKind.BABY_BATTERY_LOW),
                    b(BannerKind.READINESS),
                    b(BannerKind.MIC_SILENT),
                )
            )
        assertEquals(listOf(BannerKind.MIC_SILENT, BannerKind.READINESS), kinds(s.shown))
        assertEquals(listOf(BannerKind.BABY_BATTERY_LOW, BannerKind.VIBRATE_ONLY), kinds(s.folded))
    }

    @Test
    fun oneTwoAndThreeBanners() {
        assertEquals(1, rankBanners(listOf(b(BannerKind.PAUSED))).shown.size)
        assertEquals(0, rankBanners(listOf(b(BannerKind.PAUSED))).folded.size)
        val two = rankBanners(listOf(b(BannerKind.PAUSED), b(BannerKind.VIBRATE_ONLY)))
        assertEquals(2, two.shown.size)
        assertEquals(0, two.folded.size)
        val three =
            rankBanners(
                listOf(b(BannerKind.PAUSED), b(BannerKind.VIBRATE_ONLY), b(BannerKind.SECURITY))
            )
        assertEquals(listOf(BannerKind.SECURITY, BannerKind.PAUSED), kinds(three.shown))
        assertEquals(listOf(BannerKind.VIBRATE_ONLY), kinds(three.folded))
    }

    @Test
    fun pauseRanksAboveVibrateOnlyAndWeakConnectionAbovePause() {
        val s =
            rankBanners(
                listOf(
                    b(BannerKind.VIBRATE_ONLY),
                    b(BannerKind.PAUSED),
                    b(BannerKind.WEAK_CONNECTION),
                ),
                limit = 3,
            )
        assertEquals(
            listOf(BannerKind.WEAK_CONNECTION, BannerKind.PAUSED, BannerKind.VIBRATE_ONLY),
            kinds(s.shown),
        )
    }

    @Test
    fun securityComesBeforeEverything() {
        val all = BannerKind.entries.reversed().map { b(it) }
        assertEquals(BannerKind.SECURITY, rankBanners(all).shown.first().kind)
        assertEquals(BannerKind.entries.toList(), kinds(rankBanners(all, limit = 99).shown))
    }

    @Test
    fun bannersOfOneKindKeepTheirOrder() {
        val s =
            rankBanners(
                listOf(
                    b(BannerKind.UNIT_WARNING, "unit_warning_wake_lock_off"),
                    b(BannerKind.UNIT_WARNING, "unit_warning_page_hidden"),
                ),
                limit = 5,
            )
        assertEquals(
            listOf("unit_warning_wake_lock_off", "unit_warning_page_hidden"),
            s.shown.map { it.id },
        )
    }

    @Test
    fun levelsDecideWhatIsAnnounced() {
        assertEquals(BannerLevel.ALERT, bannerLevel(BannerKind.SECURITY))
        assertEquals(BannerLevel.WARN, bannerLevel(BannerKind.MIC_SILENT))
        assertEquals(BannerLevel.WARN, bannerLevel(BannerKind.READINESS))
        assertEquals(BannerLevel.INFO, bannerLevel(BannerKind.PAUSED))
        assertEquals(BannerLevel.NOTE, bannerLevel(BannerKind.VIBRATE_ONLY))
    }

    // ---- which notices the state has ------------------------------------------------------

    private val connected = LinkUi(phase = LinkPhase.CONNECTED)

    private fun state(
        peer: PeerStatusUi = PeerStatusUi(),
        link: LinkUi = connected,
        readiness: List<ReadinessIssue> = emptyList(),
        snoozedUntil: Long? = null,
        security: Boolean = false,
    ) =
        ParentMonitorState(
            link = link,
            peer = peer,
            readiness = readiness,
            snoozedUntilWallMs = snoozedUntil,
            securityWarning = security,
        )

    private fun ids(
        p: ParentMonitorState,
        style: AlertStyle = AlertStyle.SOUND,
        now: Long = 1_000L,
    ) = parentBanners(p, style, now).map { it.id }

    @Test
    fun aQuietConnectedMonitorHasNoBanner() {
        assertEquals(
            emptyList<String>(),
            ids(state(PeerStatusUi(batteryPct = 80, charging = false))),
        )
    }

    @Test
    fun microphoneSilentAndTheSecurityWarning() {
        assertEquals(listOf("mic_silent"), ids(state(PeerStatusUi(micSilent = true))))
        assertEquals(
            emptyList<String>(),
            ids(
                state(PeerStatusUi(micSilent = true), link = LinkUi(phase = LinkPhase.RECONNECTING))
            ),
        )
        assertEquals(listOf("security"), ids(state(security = true)))
        assertEquals(
            listOf("mic_permission_lost"),
            ids(state(PeerStatusUi(unitWarnings = listOf("mic_permission_lost")))),
        )
    }

    @Test
    fun readinessShowsOnlyTheWorstItem() {
        val p =
            state(
                readiness =
                    listOf(ReadinessIssue.NOTIFICATIONS_BLOCKED, ReadinessIssue.PARENT_BATTERY_LOW)
            )
        assertEquals(listOf("notifications_blocked"), ids(p))
        assertEquals(
            listOf("alarm_volume_low"),
            ids(state(readiness = listOf(ReadinessIssue.ALARM_VOLUME_LOW))),
        )
    }

    @Test
    fun babyBatteryThresholds() {
        fun low(pct: Int, charging: Boolean = false) =
            ids(state(PeerStatusUi(batteryPct = pct, charging = charging)))
                .contains("baby_battery_low")
        assertEquals(15, BabyBattery.LOW_PCT)
        assertEquals(5, BabyBattery.CRITICAL_PCT)
        assertEquals(false, low(4)) // critical: the full-screen alert, not a banner
        assertEquals(true, low(5))
        assertEquals(true, low(14))
        assertEquals(false, low(15))
        assertEquals(false, low(14, charging = true))
    }

    @Test
    fun silenceUnitWarningsAndWeakConnection() {
        // `not_charging` stays a device-check fact; it never becomes a monitor banner.
        assertEquals(
            emptyList<String>(),
            ids(state(PeerStatusUi(unitWarnings = listOf("not_charging")))),
        )
        assertEquals(
            listOf("silence"),
            ids(state(PeerStatusUi(silence = SilenceStatus.MAY_RING))),
        )
        assertEquals(
            emptyList<String>(),
            ids(state(PeerStatusUi(silence = SilenceStatus.MAY_RING, isWebClient = true))),
        )
        assertEquals(
            listOf("unit_warning_page_hidden", "unit_warning_wake_lock_off"),
            ids(state(PeerStatusUi(unitWarnings = listOf("page_hidden", "wake_lock_off")))),
        )
        assertEquals(
            listOf("weak_connection"),
            ids(state(link = LinkUi(phase = LinkPhase.CONNECTED, quality = LinkQuality.POOR))),
        )
    }

    @Test
    fun pauseAndVibrateOnly() {
        assertEquals(listOf("paused"), ids(state(snoozedUntil = 5_000L), now = 1_000L))
        assertEquals(emptyList<String>(), ids(state(snoozedUntil = 5_000L), now = 5_000L))
        assertEquals(listOf("vibrate_only"), ids(state(), AlertStyle.DISCREET))
    }

    // ---- level history ---------------------------------------------------------------------

    @Test
    fun levelHistoryBuckets() {
        val h = LevelHistory()
        assertEquals(List(60) { 0f }, h.bars(120_000))
        h.push(60_000, 0.3f)
        h.push(61_000, 0.4f)
        h.push(120_000, 0.5f)
        h.push(59_999, 0.9f)
        val bars = h.bars(120_000)
        assertEquals(60, bars.size)
        assertEquals(0.3f, bars[0])
        assertEquals(0.4f, bars[1])
        assertEquals(0.5f, bars[59])
        assertEquals(0.0f, bars[2])
    }

    @Test
    fun aBucketKeepsTheLoudestAndClearEmpties() {
        val h = LevelHistory()
        h.push(100_000, 0.2f)
        h.push(100_300, 0.7f)
        h.push(100_600, 0.1f)
        assertEquals(0.7f, h.bars(110_000).max())
        h.clear()
        assertEquals(0f, h.bars(110_000).max())
    }
}
