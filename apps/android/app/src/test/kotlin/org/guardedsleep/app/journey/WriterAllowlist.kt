package org.guardedsleep.app.journey

/**
 * Fields of the UI state models that no journey writes to a non-default value, each with the
 * reason. An entry that a journey starts writing must be removed (the writer check says so), and an
 * entry for a field that no longer exists fails too.
 *
 * Backlog pointers are entries of `docs/backlog.md` (IDEA) found by `ui-journey-tests`.
 */
object WriterAllowlist {
    private const val TALK =
        "talk-back needs an audio path the fake media does not model (parent talk microphone, " +
            "baby playout lease); proven by TalkModelTest, TalkSendControlTest, TalkPlayoutTest, " +
            "LinkControllerTest and the emulator gate"
    private const val NO_STATS = "comes from inbound audio statistics; the fake media reports none"
    private const val STALE =
        "needs the baby unit absent from the relay for ten minutes; the fake relay keeps its member"
    private const val HINTS =
        "calm hints the core raises for conditions no journey creates; the UI renders none yet " +
            "(UiContract.kt)"
    private const val SECURITY =
        "needs a peer that proves a different parent secret; covered at platform level by " +
            "LinkControllerTest (parent pinning)"
    private const val NOT_DRIVEN = "no journey drives it yet (candidate for a later journey)"

    val entries: Map<String, String> =
        mapOf(
            "AppState.backgroundProtection" to
                "false only after the foreground service was refused or lost; Robolectric cannot " +
                    "model service start rules (emulator gate)",
            "AppState.coachPending" to
                "product gap: the first-run coach is never wired (docs/backlog.md IDEA \"First-run " +
                    "coach marks\"); the journey for it is @Ignored",
            "AppState.lastUsedWallMs" to
                "product gap: LinkController never sets it, so the home card's \"last used\" line " +
                    "never shows (docs/backlog.md IDEA \"Last used\" line on the home card)",
            "BabyMonitorState.lastTalk.atWallMs" to TALK,
            "BabyMonitorState.lastTalk.durationMs" to TALK,
            "BabyMonitorState.lastTalk.name" to TALK,
            "BabyMonitorState.parentTalking" to TALK,
            "ParentMonitorState.afterTalkListenUntilElapsedMs" to TALK,
            "ParentMonitorState.noisePause" to TALK,
            "ParentMonitorState.talkAudibleUncertain" to TALK,
            "ParentMonitorState.talkEnd" to TALK,
            "ParentMonitorState.talkMicSilent" to TALK,
            "ParentMonitorState.talkPermission" to TALK,
            "ParentMonitorState.talkUnconfirmed" to TALK,
            "BabyMonitorState.link.acknowledgedUntilElapsedMs" to
                "acknowledging an alarm exists on the parent's link only",
            "BabyMonitorState.link.causeHint" to
                "a journey's outage is reported on the parent's link; the baby's link gets no cause",
            "BabyMonitorState.link.hints" to HINTS,
            "ParentMonitorState.link.hints" to HINTS,
            "BabyMonitorState.link.quality" to "link quality $NO_STATS",
            "ParentMonitorState.link.quality" to "link quality $NO_STATS",
            "BabyMonitorState.link.stalePairingHint" to STALE,
            "ParentMonitorState.link.stalePairingHint" to STALE,
            "BabyMonitorState.micSilent" to
                "needs microphone samples of exactly zero for the silent-mic window (BabyModelTest)",
            "BabyMonitorState.securityWarning" to SECURITY,
            "ParentMonitorState.securityWarning" to SECURITY,
            "BabyMonitorState.silenceRestoredNotice" to
                "needs do-not-disturb state left over by a killed process; the fake silence " +
                    "gateway restores nothing (emulator gate)",
            "ParentMonitorState.missed.endWallMs" to
                "no journey ends an outage with the baby unit replaying its event log",
            "ParentMonitorState.missed.loudest" to
                "no journey ends an outage with the baby unit replaying its event log",
            "ParentMonitorState.noiseEscalatedAfterMin" to
                "needs the discreet style and unacknowledged minutes (DiscreetAlertTest)",
            "ParentMonitorState.peer.nextAlarmMinutes" to
                "the fake silence gateway reports no alarm clock",
            "ParentMonitorState.peer.unitWarnings" to
                "the baby unit's own warnings come from device facts the fakes keep healthy",
            "ParentMonitorState.silenceRequest" to
                "\"silence the baby unit\" from the parent is $NOT_DRIVEN",
        )
}
