package org.guardedsleep.app.ui.common

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.withTimeoutOrNull
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.NoisePause
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.TalkDeny
import org.guardedsleep.app.ui.model.TalkEnd
import org.guardedsleep.app.ui.model.TalkPermission
import org.guardedsleep.app.ui.model.TalkUi
import org.guardedsleep.app.ui.model.TalkUnavailable
import org.guardedsleep.app.ui.theme.LocalAppColors

/** Hold time before a press counts (listen: 150 ms): a deliberate hold, never a stray tap. */
private const val HOLD_MS = 300L

/** The last seconds of the limit get a countdown and a haptic. */
private const val COUNTDOWN_SECONDS = 5

private fun secondsLeft(untilMs: Long, nowMs: Long): Int =
    (((untilMs - nowMs).coerceAtLeast(0) + 999) / 1000).toInt()

/** The reason line for a disabled button or a denial, null when there is nothing to say. */
@Composable
private fun reasonText(talk: TalkUi, elapsed: Long): String? =
    when (talk) {
        is TalkUi.Unavailable ->
            when (talk.reason) {
                TalkUnavailable.OFF -> stringResource(R.string.native_ui_talk_msg_off)
                TalkUnavailable.OTHER_TALKING -> stringResource(R.string.native_ui_talk_msg_busy)
                TalkUnavailable.UNSUPPORTED ->
                    stringResource(R.string.native_ui_talk_msg_unsupported)
                TalkUnavailable.MUTED -> stringResource(R.string.native_ui_talk_msg_muted)
                TalkUnavailable.NOT_CONNECTED ->
                    stringResource(R.string.native_ui_talk_msg_not_connected)
                TalkUnavailable.COOLDOWN ->
                    stringResource(
                        R.string.native_ui_talk_msg_cooldown,
                        secondsLeft(talk.retryAtElapsedMs ?: elapsed, elapsed),
                    )
            }
        is TalkUi.Denied ->
            when (talk.reason) {
                TalkDeny.OFF -> stringResource(R.string.native_ui_talk_msg_off)
                TalkDeny.UNSUPPORTED -> stringResource(R.string.native_ui_talk_msg_unsupported)
                TalkDeny.MUTED -> stringResource(R.string.native_ui_talk_msg_muted)
                TalkDeny.BUSY -> stringResource(R.string.native_ui_talk_msg_busy)
                TalkDeny.COOLDOWN,
                TalkDeny.BUDGET ->
                    stringResource(
                        R.string.native_ui_talk_msg_cooldown,
                        secondsLeft(talk.retryAtElapsedMs ?: elapsed, elapsed),
                    )
                TalkDeny.RATE -> stringResource(R.string.native_ui_talk_msg_rate)
                TalkDeny.UNKNOWN -> stringResource(R.string.native_ui_talk_msg_unknown)
            }
        else -> null
    }

@Composable
private fun endText(end: TalkEnd?): String? =
    when (end) {
        TalkEnd.LIMIT -> stringResource(R.string.native_ui_talk_msg_end_limit)
        TalkEnd.NO_ANSWER -> stringResource(R.string.native_ui_talk_msg_end_no_answer)
        TalkEnd.LINK_LOST -> stringResource(R.string.native_ui_talk_msg_end_link_lost)
        TalkEnd.MIC_FAILED -> stringResource(R.string.native_ui_talk_msg_end_mic_failed)
        TalkEnd.ALERT -> stringResource(R.string.native_ui_talk_msg_end_alert)
        TalkEnd.SWITCHED_OFF -> stringResource(R.string.native_ui_talk_msg_end_switched_off)
        TalkEnd.RELEASED,
        TalkEnd.STOPPED,
        null -> null
    }

/**
 * The parent's push-to-talk tile in the dock (design section 9): a 300 ms hold, states "One
 * moment…" and "Speak now", haptics at the grant, 5 s before the limit and at the cut, and no live
 * regions: while the microphone is open nothing may be announced into it. A talk that cannot start
 * before a known time says "Talk again at {time}" as an outlined status slot at full contrast,
 * never as a disabled button. Its reasons and notices are [TalkStatus].
 */
@Composable
fun TalkTile(
    p: ParentMonitorState,
    elapsed: Long,
    wall: Long,
    onPress: (Boolean) -> Unit,
    onHint: (Int?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val talk = p.talk
    val enabled = talk !is TalkUi.Unavailable
    val haptic = LocalHapticFeedback.current
    val left = (talk as? TalkUi.Talking)?.let { secondsLeft(it.untilElapsedMs, elapsed) }
    val counting = talk is TalkUi.Talking && left != null && left <= COUNTDOWN_SECONDS

    // Haptics replace speech while the microphone is open: grant, 5 s left, and the cut.
    LaunchedEffect(talk is TalkUi.Talking) {
        if (talk is TalkUi.Talking) haptic.performHapticFeedback(HapticFeedbackType.LongPress)
    }
    LaunchedEffect(left == COUNTDOWN_SECONDS) {
        if (left == COUNTDOWN_SECONDS)
            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
    }
    LaunchedEffect(p.talkEnd) {
        if (p.talkEnd == TalkEnd.LIMIT) haptic.performHapticFeedback(HapticFeedbackType.LongPress)
    }

    val retryAt =
        (talk as? TalkUi.Unavailable)
            ?.takeIf { it.reason == TalkUnavailable.COOLDOWN }
            ?.retryAtElapsedMs
    val againAt = retryAt?.let { formatTime(elapsedToWall(it, elapsed, wall)) }
    val label =
        stringResource(
            when {
                againAt != null -> R.string.native_ui_talk_again
                talk is TalkUi.Requesting -> R.string.native_ui_talk_requesting
                counting -> R.string.native_ui_talk_release_in
                talk is TalkUi.Talking -> R.string.native_ui_talk_speaking
                talk is TalkUi.Unavailable && talk.reason == TalkUnavailable.NOT_CONNECTED ->
                    R.string.native_ui_talk_disabled
                else -> R.string.native_ui_talk_idle
            },
            *(if (counting && left != null) arrayOf<Any>(left) else emptyArray()),
        )
    val sub = againAt?.let { stringResource(R.string.native_ui_talk_again_at, it) }
    val idleLabel = stringResource(R.string.native_ui_talk_idle)
    val scheme = MaterialTheme.colorScheme
    val status = againAt != null
    val container =
        when {
            status -> Color.Transparent
            !enabled -> scheme.surfaceContainerLow
            talk is TalkUi.Talking -> scheme.tertiary
            else -> scheme.tertiaryContainer
        }
    val content =
        when {
            status -> scheme.onSurface
            !enabled -> LocalAppColors.current.secondaryText
            talk is TalkUi.Talking -> scheme.onTertiary
            else -> scheme.onTertiaryContainer
        }
    val onPressState = rememberUpdatedState(onPress)
    val enabledState = rememberUpdatedState(enabled)
    val onHintState = rememberUpdatedState(onHint)
    val lockPx = with(LocalDensity.current) { 72.dp.toPx() }
    val shape = RoundedCornerShape(22.dp)

    Column(
        modifier
            .heightIn(min = 72.dp)
            .background(container, shape)
            .then(
                when {
                    // A filled ring while the microphone is open (static: no pulse to reduce).
                    talk is TalkUi.Talking ->
                        Modifier.border(BorderStroke(4.dp, scheme.onTertiary), shape)
                    status -> Modifier.border(BorderStroke(1.5.dp, scheme.outline), shape)
                    else -> Modifier
                }
            )
            .semantics(mergeDescendants = true) {
                role = Role.Button
                // Constant while talking: a changing description would be announced.
                stateDescription = idleLabel
                if (!enabled) disabled()
            }
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    if (!enabledState.value) {
                        waitForUpOrCancellation()
                        return@awaitEachGesture
                    }
                    val early = withTimeoutOrNull(HOLD_MS) { waitForUpOrCancellation() }
                    if (early != null) {
                        // A tap, not a hold.
                        onHintState.value(R.string.native_ui_talk_tap_hint)
                        return@awaitEachGesture
                    }
                    if (currentEvent.changes.none { it.pressed }) return@awaitEachGesture
                    onHintState.value(null)
                    onPressState.value(true)
                    try {
                        while (true) {
                            val change = awaitPointerEvent().changes.firstOrNull() ?: break
                            if (!change.pressed) break
                            if (change.position.y - down.position.y <= -lockPx)
                                onHintState.value(R.string.native_ui_talk_lock_hint)
                            change.consume()
                        }
                    } finally {
                        // Release, a lifted finger that slid off, or a cancelled gesture: the
                        // talk always ends with the touch.
                        onPressState.value(false)
                    }
                }
            }
            .padding(horizontal = 4.dp, vertical = 8.dp)
            .testTag("parent_talk"),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        GlyphIcon(Glyph.MIC, content, size = 26.dp)
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = content,
            textAlign = TextAlign.Center,
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Ellipsis,
        )
        if (sub != null)
            Text(
                sub,
                style = MaterialTheme.typography.labelMedium,
                color = content,
                textAlign = TextAlign.Center,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Ellipsis,
            )
    }
}

/**
 * What the talk panel says above the dock: the status line "Listening is paused while you talk",
 * why talk is unavailable or was denied, how the last talk ended, the noise pause, the silent
 * microphone and unconfirmed notices, the coach mark and the 10 s listen after a talk. A visible
 * text only: no live region, so nothing is spoken into the open microphone.
 */
@Composable
fun TalkStatus(
    p: ParentMonitorState,
    elapsed: Long,
    hint: Int?,
    coachVisible: Boolean,
    onKeepListening: () -> Unit,
    onPermissionResult: (Boolean) -> Unit,
    onPermissionDismiss: () -> Unit,
    onCoachDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val talk = p.talk
    val active = talk is TalkUi.Requesting || talk is TalkUi.Talking
    Column(
        modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (coachVisible) {
            AppCard(Modifier.testTag("coach_talk"), onClick = onCoachDismiss) {
                Text(
                    stringResource(R.string.native_ui_talk_coach),
                    style = MaterialTheme.typography.bodyLarge,
                )
            }
        }
        if (active) {
            // A status, not a disabled control: detection is paused on the baby device meanwhile.
            Body(
                stringResource(R.string.native_ui_talk_paused_listening),
                tag = "parent_talk_listening_paused",
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
        val reason = reasonText(talk, elapsed)
        if (reason != null) {
            Body(
                reason,
                tag =
                    if (talk is TalkUi.Unavailable) "parent_talk_unavailable_reason"
                    else "parent_talk_state",
                color =
                    if (talk is TalkUi.Denied) MaterialTheme.colorScheme.error
                    else LocalAppColors.current.secondaryText,
            )
        }
        endText(p.talkEnd)?.let {
            Body(it, tag = "parent_talk_state", color = LocalAppColors.current.secondaryText)
        }
        hint?.let { Body(stringResource(it), tag = "parent_talk_hint") }
        when (p.noisePause) {
            NoisePause.YOU ->
                Body(
                    stringResource(R.string.native_ui_talk_msg_noise_paused_you),
                    tag = "parent_noise_paused",
                )
            NoisePause.OTHER ->
                Body(
                    stringResource(R.string.native_ui_talk_msg_noise_paused_other),
                    tag = "parent_noise_paused",
                )
            null -> Unit
        }
        if (p.talkMicSilent && talk is TalkUi.Talking)
            Body(
                stringResource(R.string.native_ui_talk_msg_mic_silent),
                tag = "parent_talk_mic_silent",
            )
        if (p.talkUnconfirmed)
            Body(
                stringResource(R.string.native_ui_talk_msg_unconfirmed),
                tag = "parent_talk_unconfirmed",
            )
        else if (p.talkAudibleUncertain && !active)
            Body(
                stringResource(R.string.native_ui_talk_msg_uncertain),
                tag = "parent_talk_unconfirmed",
            )
        p.afterTalkListenUntilElapsedMs?.let { until ->
            Text(
                stringResource(
                    R.string.native_ui_talk_after_listen,
                    secondsLeft(until, elapsed),
                ),
                Modifier.clickable(onClick = onKeepListening)
                    .padding(8.dp)
                    .testTag("parent_talk_after_listen"),
                style = MaterialTheme.typography.bodyLarge,
                color = LocalAppColors.current.ok,
                textAlign = TextAlign.Center,
            )
        }
    }
    if (p.talkPermission != TalkPermission.NONE) {
        TalkPermissionSheet(p.talkPermission, onPermissionResult, onPermissionDismiss)
    }
}

/**
 * The separate microphone step before the first talk (never during a hold): why it is needed, then
 * the system dialog, then "Done" or the way to the device settings.
 */
@Composable
private fun TalkPermissionSheet(
    state: TalkPermission,
    onResult: (Boolean) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val launcher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission(), onResult)
    SheetOverlay(
        "parent_talk_permission_sheet",
        stringResource(R.string.native_ui_talk_idle),
        onDismiss,
    ) {
        when (state) {
            TalkPermission.NEEDED -> {
                Body(stringResource(R.string.native_ui_talk_perm_body))
                PrimaryButton(
                    stringResource(R.string.native_ui_talk_perm_allow),
                    { launcher.launch(android.Manifest.permission.RECORD_AUDIO) },
                    "parent_talk_permission_allow",
                    Modifier.fillMaxWidth(),
                )
            }
            TalkPermission.JUST_GRANTED -> {
                Body(
                    stringResource(R.string.native_ui_talk_perm_done),
                    tag = "parent_talk_permission_done",
                )
                PrimaryButton(
                    stringResource(R.string.native_ui_common_ok),
                    onDismiss,
                    "parent_talk_permission_ok",
                    Modifier.fillMaxWidth(),
                )
            }
            TalkPermission.REFUSED -> {
                Body(stringResource(R.string.native_ui_talk_perm_refused))
                PrimaryButton(
                    stringResource(R.string.native_ui_talk_perm_settings),
                    {
                        SystemSettings.launch(ctx, SystemSettings.appDetails(ctx))
                        onDismiss()
                    },
                    "parent_talk_permission_settings",
                    Modifier.fillMaxWidth(),
                )
            }
            TalkPermission.NONE -> Unit
        }
    }
}

/**
 * Ends a talk when the app leaves the foreground, the screen turns off or a call starts
 * (`talk-back` "Talk ends when the hold is interrupted"). Touch cancellation ends it in the button
 * itself.
 */
@Composable
fun TalkInterruptEffect(onInterrupt: () -> Unit) {
    val ctx = LocalContext.current
    val owner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    val interrupt = rememberUpdatedState(onInterrupt)
    androidx.compose.runtime.DisposableEffect(owner, ctx) {
        val observer =
            androidx.lifecycle.LifecycleEventObserver { _, e ->
                if (e == androidx.lifecycle.Lifecycle.Event.ON_STOP) interrupt.value()
            }
        owner.lifecycle.addObserver(observer)
        val off =
            object : android.content.BroadcastReceiver() {
                override fun onReceive(c: android.content.Context?, i: android.content.Intent?) =
                    interrupt.value()
            }
        androidx.core.content.ContextCompat.registerReceiver(
            ctx,
            off,
            android.content.IntentFilter(android.content.Intent.ACTION_SCREEN_OFF),
            androidx.core.content.ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        val stopCallWatch: () -> Unit =
            if (android.os.Build.VERSION.SDK_INT >= 31) watchCalls(ctx) { interrupt.value() }
            else {
                // Before Android 12 the call screen takes the foreground, so `onStop` covers it.
                {}
            }
        onDispose {
            owner.lifecycle.removeObserver(observer)
            runCatching { ctx.unregisterReceiver(off) }
            stopCallWatch()
        }
    }
}

/** A call (or a ringing phone) changes the audio mode; returns the function that stops watching. */
@androidx.annotation.RequiresApi(31)
private fun watchCalls(ctx: android.content.Context, onCall: () -> Unit): () -> Unit {
    val am = ctx.getSystemService(android.media.AudioManager::class.java) ?: return {}
    val listener =
        android.media.AudioManager.OnModeChangedListener { m ->
            if (
                m == android.media.AudioManager.MODE_IN_CALL ||
                    m == android.media.AudioManager.MODE_RINGTONE ||
                    m == android.media.AudioManager.MODE_CALL_SCREENING
            )
                onCall()
        }
    am.addOnModeChangedListener(androidx.core.content.ContextCompat.getMainExecutor(ctx), listener)
    return { am.removeOnModeChangedListener(listener) }
}
