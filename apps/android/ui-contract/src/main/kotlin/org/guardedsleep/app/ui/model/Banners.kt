package org.guardedsleep.app.ui.model

/*
 * Which notices the parent monitor shows and in which order (`parent-alerts` "Banner priority on
 * the monitor"). Presentation only: every fact comes from the contract models and nothing here
 * decides about the link, an alert or a timer. The web client mirrors this file in
 * `apps/web/src/lib/ui/banners.ts`; a contract mirror test compares the order.
 */

/** At most this many banners show; the rest fold into one "+N more" row. */
const val BANNER_LIMIT = 2

/**
 * The kinds of banner, in the order they rank: the declaration order is the priority (security
 * first, "Vibrate only" last). The web client adds three browser kinds right below [READINESS].
 */
enum class BannerKind {
    SECURITY,
    MIC_SILENT,
    READINESS,
    BABY_BATTERY_LOW,
    BABY_UNPLUGGED,
    SILENCE,
    UNIT_WARNING,
    WEAK_CONNECTION,
    PAUSED,
    VIBRATE_ONLY,
}

/** How loud a banner is; each new banner of [WARN] or higher is announced once. */
enum class BannerLevel {
    NOTE,
    INFO,
    WARN,
    ALERT,
}

/** One banner; [id] tells apart banners of one kind (the readiness item, the unit warning). */
data class Banner(val kind: BannerKind, val id: String = kind.name.lowercase())

/** The banners that show and the ones folded into "+N more", each in rank order. */
data class BannerStack(val shown: List<Banner>, val folded: List<Banner>)

fun bannerLevel(kind: BannerKind): BannerLevel =
    when (kind) {
        BannerKind.SECURITY -> BannerLevel.ALERT
        BannerKind.MIC_SILENT,
        BannerKind.READINESS,
        BannerKind.BABY_BATTERY_LOW,
        BannerKind.SILENCE,
        BannerKind.UNIT_WARNING,
        BannerKind.WEAK_CONNECTION -> BannerLevel.WARN
        BannerKind.BABY_UNPLUGGED,
        BannerKind.PAUSED -> BannerLevel.INFO
        BannerKind.VIBRATE_ONLY -> BannerLevel.NOTE
    }

/** Stable by rank: banners of one kind keep the order they were given in. */
fun rankBanners(banners: List<Banner>, limit: Int = BANNER_LIMIT): BannerStack {
    val ranked = banners.sortedBy { it.kind.ordinal }
    return BannerStack(ranked.take(limit), ranked.drop(limit))
}

/** The unit warnings that become banners of their own kind (the rest have their own place). */
private val UNIT_WARNING_BANNERS = listOf("page_hidden", "wake_lock_off", "resumed_from_sleep")

/**
 * Every notice the monitor has right now, in no particular order; [rankBanners] orders them. The
 * readiness banner is the worst failing item (the checker lists them worst first). Facts that the
 * baby device last reported (battery, silence, warnings) count only while connected, so a banner
 * never states something the parent can no longer verify.
 */
fun parentBanners(p: ParentMonitorState, alertStyle: AlertStyle, wallNowMs: Long): List<Banner> {
    val out = mutableListOf<Banner>()
    val connected = p.link.phase == LinkPhase.CONNECTED
    if (p.securityWarning) out += Banner(BannerKind.SECURITY)
    if (connected && p.peer.micSilent == true) out += Banner(BannerKind.MIC_SILENT, "mic_silent")
    if (connected && "mic_permission_lost" in p.peer.unitWarnings)
        out += Banner(BannerKind.MIC_SILENT, "mic_permission_lost")
    p.readiness.firstOrNull()?.let {
        out += Banner(BannerKind.READINESS, it.name.lowercase())
    }
    val pct = p.peer.batteryPct
    if (
        connected &&
            pct != null &&
            pct >= BabyBattery.CRITICAL_PCT &&
            pct < BabyBattery.LOW_PCT &&
            p.peer.charging != true
    )
        out += Banner(BannerKind.BABY_BATTERY_LOW)
    if (connected && "not_charging" in p.peer.unitWarnings) out += Banner(BannerKind.BABY_UNPLUGGED)
    if (connected && !p.peer.isWebClient) {
        when (p.peer.silence) {
            SilenceStatus.SCREEN_MAY_LIGHT,
            SilenceStatus.MAY_BUZZ,
            SilenceStatus.MAY_RING,
            SilenceStatus.NO_ACCESS -> out += Banner(BannerKind.SILENCE)
            SilenceStatus.SILENT,
            SilenceStatus.UNKNOWN -> Unit
        }
    }
    if (connected)
        for (w in p.peer.unitWarnings) if (w in UNIT_WARNING_BANNERS)
            out += Banner(BannerKind.UNIT_WARNING, "unit_warning_$w")
    if (connected && p.link.quality == LinkQuality.POOR) out += Banner(BannerKind.WEAK_CONNECTION)
    if ((p.snoozedUntilWallMs ?: 0L) > wallNowMs) out += Banner(BannerKind.PAUSED)
    if (alertStyle == AlertStyle.DISCREET) out += Banner(BannerKind.VIBRATE_ONLY)
    return out
}
