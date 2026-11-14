package org.guardedsleep.app.ui.common

import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkUi

/*
 * What the parent's status bar and connection alerts show, derived from the contract model.
 * Colour and words follow the core's state and level only; nothing here has a threshold or a timer
 * of its own (AGENTS.md section 5.1). The web client mirrors this table in `parent-view.ts`.
 */

/** The stage of the link as the bar words it (`connection-status` "Ambient bar per level"). */
enum class StatusKind {
    CONNECTED,
    /** `connecting`, level 0: blue. */
    CONNECTING,
    /** `reconnecting`, level 0: blue. */
    RECONNECTING,
    /** `connecting`, level 1: orange, "Not connected yet". */
    NOT_CONNECTED_YET,
    /** `reconnecting`, level 1: orange, "Can't reach". */
    CANT_REACH,
    /** `connecting`, level 2: red, monitoring never started. */
    COULDNT_CONNECT,
    /** `reconnecting`, level 2: red. */
    NO_CONNECTION,
    /** Level 3: red with the loud alarm. */
    STILL_NO_CONNECTION,
    STOPPED,
    NEEDS_USER,
    IDLE,
}

/** Fill of the bar; the safety colours behind them never change with the scheme. */
enum class StatusStyle {
    CALM,
    INFO,
    WARN,
    ALERT,
    GREY,
}

fun statusKindOf(link: LinkUi): StatusKind =
    when (link.phase) {
        LinkPhase.CONNECTED -> StatusKind.CONNECTED
        LinkPhase.PEER_STOPPED -> StatusKind.STOPPED
        LinkPhase.FAILED_NEEDS_USER -> StatusKind.NEEDS_USER
        LinkPhase.IDLE,
        LinkPhase.STOPPED -> StatusKind.IDLE
        LinkPhase.CONNECTING,
        LinkPhase.RECONNECTING -> {
            val first = link.phase == LinkPhase.CONNECTING
            when {
                link.level >= 3 -> StatusKind.STILL_NO_CONNECTION
                link.level == 2 ->
                    if (first) StatusKind.COULDNT_CONNECT else StatusKind.NO_CONNECTION
                link.level == 1 ->
                    if (first) StatusKind.NOT_CONNECTED_YET else StatusKind.CANT_REACH
                else -> if (first) StatusKind.CONNECTING else StatusKind.RECONNECTING
            }
        }
    }

fun StatusKind.style(): StatusStyle =
    when (this) {
        StatusKind.CONNECTED -> StatusStyle.CALM
        StatusKind.CONNECTING,
        StatusKind.RECONNECTING -> StatusStyle.INFO
        StatusKind.NOT_CONNECTED_YET,
        StatusKind.CANT_REACH -> StatusStyle.WARN
        StatusKind.COULDNT_CONNECT,
        StatusKind.NO_CONNECTION,
        StatusKind.STILL_NO_CONNECTION,
        StatusKind.NEEDS_USER -> StatusStyle.ALERT
        StatusKind.STOPPED,
        StatusKind.IDLE -> StatusStyle.GREY
    }

/**
 * Stage changes from level 2 are announced assertively (`connection-status` "State announcements").
 */
fun StatusKind.assertive(): Boolean =
    when (this) {
        StatusKind.COULDNT_CONNECT,
        StatusKind.NO_CONNECTION,
        StatusKind.STILL_NO_CONNECTION,
        StatusKind.STOPPED,
        StatusKind.NEEDS_USER -> true
        else -> false
    }

/** The outage start on the elapsed clock; the last contact when the core sent no start. */
fun LinkUi.outageStartElapsedMs(): Long? = downSinceElapsedMs ?: lastAliveElapsedMs

/** The three stages of the full-screen connection alert (`connection-status`). */
enum class AlertStage {
    VIBRATING,
    LOUD,
    PAUSED,
}

fun alertStageOf(link: LinkUi, elapsedNow: Long): AlertStage =
    when {
        (link.acknowledgedUntilElapsedMs ?: 0L) > elapsedNow -> AlertStage.PAUSED
        link.level >= 3 -> AlertStage.LOUD
        else -> AlertStage.VIBRATING
    }

/**
 * A core-clock time (elapsed realtime) as a wall-clock time, for "Loud alarm at 03:17". The offset
 * between the two clocks is constant, so the shown time does not move while the screen ticks.
 */
fun elapsedToWall(atElapsedMs: Long, elapsedNow: Long, wallNow: Long): Long =
    wallNow - (elapsedNow - atElapsedMs)
