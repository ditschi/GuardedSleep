package org.guardedsleep.app.alerts

import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.core.app.NotificationCompat
import org.guardedsleep.app.AlertActionReceiver
import org.guardedsleep.app.AlertActivity
import org.guardedsleep.app.MainActivity
import org.guardedsleep.app.R
import org.guardedsleep.app.config.AlertPolicy
import org.guardedsleep.app.diag.PocLog
import org.guardedsleep.app.ui.model.LinkPhase

/**
 * Parent-side feedback: turns [AlertSink] calls into notifications, vibration and tones using the
 * plans from [FeedbackPlans]. Level 1 is visual only; level 2 adds a full-screen alert, repeating
 * haptics and a soft tone; level 3 a continuous alarm. "I'm checking" mutes sound and vibration
 * while the notification stays. The child's audio itself is played by WebRTC, not here.
 */
class AlertController(private val context: Context) : AlertSink {
    private val nm = context.getSystemService(NotificationManager::class.java)
    private val handler = Handler(Looper.getMainLooper())
    private val haptics = Haptics(context)
    private val tones = TonePlayer()

    private var linkPlan = LinkFeedbackPlan()
    private var lastLink: LinkSnapshot? = null
    private var maxLevelInOutage = 0
    private var fgsLost = false
    private var noiseHapticLoop: Runnable? = null
    private var linkHapticLoop: Runnable? = null
    private var toneLoop: Runnable? = null

    init {
        AlertChannels.ensure(context)
    }

    override fun linkState(snapshot: LinkSnapshot) = onMain {
        lastLink = snapshot
        applyLink()
    }

    /** Link plan, raised to level 2 while the foreground service is missing. */
    private fun applyLink() {
        val snapshot = lastLink
        var plan =
            if (snapshot == null) LinkFeedbackPlan()
            else
                FeedbackPlans.forLink(
                    snapshot.phase,
                    snapshot.level,
                    snapshot.acknowledged,
                    snapshot.wasConnected,
                )
        if (fgsLost && plan.silent) plan = FeedbackPlans.forLink(LinkPhase.RECONNECTING, 2, false)
        val calm =
            snapshot == null ||
                snapshot.phase == LinkPhase.CONNECTED ||
                snapshot.phase == LinkPhase.IDLE ||
                snapshot.phase == LinkPhase.STOPPED
        if (calm && !fgsLost) {
            nm.cancel(ID_LINK_INFO)
            nm.cancel(ID_LINK_ALARM)
            stopLinkLoops()
            linkPlan = plan
            return
        }
        if (
            snapshot != null &&
                (snapshot.phase == LinkPhase.RECONNECTING || snapshot.phase == LinkPhase.CONNECTING)
        ) {
            maxLevelInOutage = maxOf(maxLevelInOutage, snapshot.level)
        }
        if (plan.notify == NotifyLevel.LOW && linkPlan.notify != NotifyLevel.LOW) {
            nm.notify(
                ID_LINK_INFO,
                linkNotification(
                    AlertChannels.INFO,
                    R.string.native_notif_status_trouble,
                    high = false,
                ),
            )
            if (plan.oneShotHaptic) haptics.play(HapticPattern.SHORT)
        }
        if (plan.notify == NotifyLevel.HIGH) {
            nm.cancel(ID_LINK_INFO)
            // Terminal failure of a link that worked: no LinkLost event comes, so raise the alarm
            // here.
            if (
                snapshot?.phase == LinkPhase.FAILED_NEEDS_USER &&
                    linkPlan.notify != NotifyLevel.HIGH
            ) {
                nm.notify(
                    ID_LINK_ALARM,
                    linkNotification(
                        AlertChannels.ALARM_LINK,
                        R.string.native_notif_status_failed,
                        high = true,
                        title = R.string.native_notif_link_lost_title,
                    ),
                )
            }
        }
        applyLinkLoops(plan)
        linkPlan = plan
    }

    override fun backgroundProtection(lost: Boolean) = onMain {
        if (fgsLost == lost) return@onMain
        fgsLost = lost
        if (lost) {
            PocLog.event("alert_shown", "kind" to "fgs_missing")
            nm.notify(
                ID_FGS,
                linkNotification(
                    AlertChannels.ALARM_LINK,
                    R.string.native_notif_fgs_missing_text,
                    high = true,
                    title = R.string.native_notif_fgs_missing_title,
                ),
            )
        } else {
            nm.cancel(ID_FGS)
        }
        applyLink()
    }

    override fun linkLost(downMs: Long) = onMain {
        PocLog.event("alert_shown", "kind" to "link_lost")
        nm.notify(
            ID_LINK_ALARM,
            linkNotification(
                AlertChannels.ALARM_LINK,
                R.string.native_notif_link_lost_text,
                high = true,
                title = R.string.native_notif_link_lost_title,
                withAck = true,
            ),
        )
    }

    override fun noiseAlert(kind: NoiseKind, probe: Boolean, snoozed: Boolean) = onMain {
        val plan = FeedbackPlans.forNoise(kind, snoozed)
        if (!plan.notify) return@onMain
        val title =
            if (probe) R.string.native_notif_noise_test_title else R.string.native_notif_noise_title
        val text =
            if (probe) R.string.native_notif_noise_test_text else R.string.native_notif_noise_text
        nm.notify(ID_NOISE, noiseNotification(title, text, plan.fullScreen))
        if (plan.repeatingHaptic) startNoiseHaptics()
        else if (plan.shortHaptic) haptics.play(HapticPattern.SHORT)
    }

    override fun noiseEnded() = onMain {
        stopNoiseHaptics()
        nm.cancel(ID_NOISE)
    }

    override fun stopNoiseSound() = onMain {
        stopNoiseHaptics()
        haptics.cancel()
    }

    override fun notice(notice: Notice) = onMain {
        when (notice) {
            Notice.PeerStopped -> {
                // One connection haptic and one soft tone, no loop (alpha-ux-flows §3.4).
                haptics.play(HapticPattern.CONNECTION)
                tones.playSoft()
                nm.notify(
                    ID_PEER_STOPPED,
                    linkNotification(
                        AlertChannels.ALARM_LINK,
                        R.string.native_notif_peer_stopped_text,
                        high = true,
                        title = R.string.native_notif_peer_stopped_title,
                    ),
                )
            }
            is Notice.LinkRestored -> {
                if (maxLevelInOutage >= 2) haptics.play(HapticPattern.SHORT)
                maxLevelInOutage = 0
                nm.cancel(ID_LINK_ALARM)
                nm.notify(
                    ID_RESTORED,
                    simple(
                        AlertChannels.INFO,
                        context.getString(R.string.native_notif_restored_title),
                        context.getString(R.string.native_notif_restored_text, mmss(notice.downMs)),
                        timeoutMs = AlertPolicy.RESTORED_NOTICE_MS,
                    ),
                )
            }
            is Notice.LowBattery -> {
                val pct = notice.pct.toString()
                if (notice.critical) {
                    haptics.play(HapticPattern.CONNECTION)
                    tones.playSoft()
                    nm.notify(
                        ID_BATTERY,
                        simple(
                            AlertChannels.ALARM_LINK,
                            context.getString(R.string.native_notif_battery_critical_title),
                            context.getString(R.string.native_notif_battery_critical_text, pct),
                        ),
                    )
                } else {
                    nm.notify(
                        ID_BATTERY,
                        simple(
                            AlertChannels.NOTICE,
                            context.getString(R.string.native_notif_battery_low_title),
                            context.getString(R.string.native_notif_battery_low_text, pct),
                        ),
                    )
                }
            }
            Notice.ChargerUnplugged ->
                nm.notify(
                    ID_CHARGER,
                    simple(
                        AlertChannels.NOTICE,
                        context.getString(R.string.native_notif_charger_title),
                        context.getString(R.string.native_notif_charger_text),
                    ),
                )
            Notice.SecurityWarning ->
                nm.notify(
                    ID_SECURITY,
                    simple(
                        AlertChannels.NOTICE,
                        context.getString(R.string.native_notif_security_title),
                        context.getString(R.string.native_notif_security_text),
                    ),
                )
            is Notice.MissedEvents -> {
                val d = mmss(notice.downMs)
                val text =
                    when (notice.count) {
                        null -> context.getString(R.string.native_notif_missed_unknown, d)
                        0 -> context.getString(R.string.native_notif_missed_none, d)
                        else ->
                            context.getString(
                                R.string.native_notif_missed_count,
                                notice.count.toString(),
                                d,
                            )
                    }
                nm.notify(
                    ID_MISSED,
                    simple(
                        AlertChannels.INFO,
                        context.getString(R.string.native_notif_missed_title),
                        text,
                    ),
                )
            }
        }
    }

    override fun clear() = onMain {
        stopLinkLoops()
        stopNoiseHaptics()
        tones.stop()
        haptics.cancel()
        listOf(
                ID_LINK_INFO,
                ID_LINK_ALARM,
                ID_NOISE,
                ID_PEER_STOPPED,
                ID_RESTORED,
                ID_BATTERY,
                ID_CHARGER,
                ID_SECURITY,
                ID_MISSED,
                ID_FGS,
            )
            .forEach(nm::cancel)
        linkPlan = LinkFeedbackPlan()
        lastLink = null
        fgsLost = false
        maxLevelInOutage = 0
    }

    // ---- loops -------------------------------------------------------------------------------

    private fun applyLinkLoops(plan: LinkFeedbackPlan) {
        if (
            plan.hapticEveryMs != linkPlan.hapticEveryMs ||
                linkHapticLoop == null && plan.hapticEveryMs != null
        ) {
            linkHapticLoop?.let(handler::removeCallbacks)
            linkHapticLoop = null
            plan.hapticEveryMs?.let { every ->
                linkHapticLoop = repeat(every) { haptics.play(HapticPattern.CONNECTION) }
            }
        }
        if (
            plan.toneEveryMs != linkPlan.toneEveryMs || toneLoop == null && plan.toneEveryMs != null
        ) {
            toneLoop?.let(handler::removeCallbacks)
            toneLoop = null
            plan.toneEveryMs?.let { every -> toneLoop = repeat(every) { tones.playSoft() } }
        }
        if (plan.continuousAlarm) tones.startAlarm() else if (linkPlan.continuousAlarm) tones.stop()
        if (plan.silent) haptics.cancel()
    }

    private fun stopLinkLoops() {
        linkHapticLoop?.let(handler::removeCallbacks)
        toneLoop?.let(handler::removeCallbacks)
        linkHapticLoop = null
        toneLoop = null
        tones.stop()
        haptics.cancel()
    }

    private fun startNoiseHaptics() {
        stopNoiseHaptics()
        noiseHapticLoop =
            repeat(AlertPolicy.NOISE_HAPTIC_INTERVAL_MS) { haptics.play(HapticPattern.NOISE) }
    }

    private fun stopNoiseHaptics() {
        noiseHapticLoop?.let(handler::removeCallbacks)
        noiseHapticLoop = null
    }

    /** Runs [action] now and every [everyMs] until its callbacks are removed. */
    private fun repeat(everyMs: Long, action: () -> Unit): Runnable {
        val r =
            object : Runnable {
                override fun run() {
                    action()
                    handler.postDelayed(this, everyMs)
                }
            }
        r.run()
        return r
    }

    private fun onMain(block: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) block() else handler.post(block)
    }

    // ---- notifications ------------------------------------------------------------------------

    private fun canFullScreen(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE || nm.canUseFullScreenIntent()

    private fun noiseNotification(title: Int, text: Int, fullScreen: Boolean): Notification {
        val builder =
            NotificationCompat.Builder(context, AlertChannels.ALARM_NOISE)
                .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
                .setContentTitle(context.getString(title))
                .setContentText(context.getString(text))
                .setCategory(NotificationCompat.CATEGORY_ALARM)
                .setPriority(NotificationCompat.PRIORITY_MAX)
                .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
                .setOnlyAlertOnce(true)
                .setPublicVersion(publicVersion(AlertChannels.ALARM_NOISE))
                .setContentIntent(activity(AlertActivity.KIND_NOISE, ID_NOISE))
                .addAction(
                    0,
                    context.getString(R.string.native_notif_action_stop_sound),
                    receiver(AlertActionReceiver.ACTION_STOP_SOUND),
                )
                .addAction(
                    0,
                    context.getString(R.string.native_notif_action_snooze10),
                    receiver(AlertActionReceiver.ACTION_SNOOZE_10),
                )
        // Without the permission (Android 14+) the notification still appears as a heads-up.
        if (fullScreen && canFullScreen())
            builder.setFullScreenIntent(activity(AlertActivity.KIND_NOISE, ID_NOISE), true)
        return builder.build()
    }

    private fun linkNotification(
        channel: String,
        text: Int,
        high: Boolean,
        title: Int = R.string.native_notif_app_title,
        withAck: Boolean = false,
    ): Notification {
        val b =
            NotificationCompat.Builder(context, channel)
                .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
                .setContentTitle(context.getString(title))
                .setContentText(context.getString(text))
                .setOngoing(high && withAck)
                .setAutoCancel(!high)
                .setOnlyAlertOnce(true)
                .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
                .setPublicVersion(publicVersion(channel))
                .setContentIntent(
                    if (high) activity(AlertActivity.KIND_LINK, ID_LINK_ALARM) else mainIntent()
                )
        if (high) {
            b.setCategory(NotificationCompat.CATEGORY_ALARM)
                .setPriority(NotificationCompat.PRIORITY_MAX)
            if (canFullScreen())
                b.setFullScreenIntent(activity(AlertActivity.KIND_LINK, ID_LINK_ALARM), true)
        } else {
            b.setPriority(NotificationCompat.PRIORITY_LOW)
        }
        if (withAck) {
            b.addAction(
                0,
                context.getString(R.string.native_notif_action_checking),
                receiver(AlertActionReceiver.ACTION_ACK),
            )
        }
        return b.build()
    }

    private fun simple(
        channel: String,
        title: String,
        text: String,
        timeoutMs: Long? = null,
    ): Notification =
        NotificationCompat.Builder(context, channel)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle(title)
            .setContentText(text)
            .setAutoCancel(true)
            .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
            .setPublicVersion(publicVersion(channel))
            .setContentIntent(mainIntent())
            .apply {
                if (channel == AlertChannels.ALARM_LINK)
                    setCategory(NotificationCompat.CATEGORY_ALARM)
                timeoutMs?.let(::setTimeoutAfter)
            }
            .build()

    /** Lock screen shows no detail (a sound level, a battery value) of an alarm. */
    private fun publicVersion(channel: String): Notification =
        NotificationCompat.Builder(context, channel)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle(context.getString(R.string.native_notif_public_title))
            .build()

    private fun mainIntent(): PendingIntent =
        PendingIntent.getActivity(
            context,
            0,
            Intent(context, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )

    private fun activity(kind: String, requestCode: Int): PendingIntent =
        PendingIntent.getActivity(
            context,
            requestCode,
            Intent(context, AlertActivity::class.java)
                .putExtra(AlertActivity.EXTRA_KIND, kind)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun receiver(action: String): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            action.hashCode(),
            Intent(context, AlertActionReceiver::class.java).setAction(action),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun mmss(ms: Long): String {
        val s = ms / 1000
        return "%d:%02d".format(s / 60, s % 60)
    }

    private companion object {
        const val ID_LINK_INFO = 2001
        const val ID_LINK_ALARM = 2002
        const val ID_FGS = 2010
        const val ID_NOISE = 2003
        const val ID_PEER_STOPPED = 2004
        const val ID_RESTORED = 2005
        const val ID_BATTERY = 2006
        const val ID_CHARGER = 2007
        const val ID_SECURITY = 2008
        const val ID_MISSED = 2009
    }
}
