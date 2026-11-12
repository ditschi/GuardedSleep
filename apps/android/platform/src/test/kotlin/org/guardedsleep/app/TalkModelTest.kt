package org.guardedsleep.app

import org.guardedsleep.app.link.BabyModel
import org.guardedsleep.app.link.ParentModel
import org.guardedsleep.app.ui.model.NoisePause
import org.guardedsleep.app.ui.model.TalkDeny
import org.guardedsleep.app.ui.model.TalkEnd
import org.guardedsleep.app.ui.model.TalkPermission
import org.guardedsleep.app.ui.model.TalkUi
import org.guardedsleep.app.ui.model.TalkUnavailable
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.DeviceStatus
import uniffi.guardedsleep_core.LinkEvent
import uniffi.guardedsleep_core.LinkState
import uniffi.guardedsleep_core.PlaybackResult
import uniffi.guardedsleep_core.TalkAudible
import uniffi.guardedsleep_core.TalkDenyReason
import uniffi.guardedsleep_core.TalkEndReason
import uniffi.guardedsleep_core.TalkState
import uniffi.guardedsleep_core.TalkStatus
import uniffi.guardedsleep_core.Talker

class TalkModelTest {
    private val clock = FakeClock()
    private val sink = RecordingSink()
    private val effects = RecordingEffects()
    private val model = ParentModel(clock, sink, effects) { 0.12 }
    private val ui
        get() = model.state.value

    private fun connected() {
        model.onStatus(status(LinkState.CONNECTING), clock.elapsed)
        model.onStatus(status(LinkState.CONNECTED), clock.elapsed)
    }

    private fun peer(
        talk: TalkStatus? = TalkStatus.ON,
        talker: Talker? = Talker.NONE,
        retry: ULong? = null,
        audible: TalkAudible? = null,
    ) =
        LinkEvent.PeerStatus(
            DeviceStatus(
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                emptyList(),
                talk,
                talker,
                null,
                retry,
                audible,
            )
        )

    private fun talk(s: TalkState) = model.onEvent(LinkEvent.TalkChanged(s), clock.elapsed)

    @Test
    fun theCoresTalkStatesReachTheButton() {
        connected()
        assertEquals(TalkUi.Idle, ui.talk)
        talk(TalkState.Requesting)
        assertEquals(TalkUi.Requesting, ui.talk)
        assertTrue(model.talkActive())
        talk(TalkState.Talking(clock.elapsed.toULong() + 30_000u))
        assertEquals(TalkUi.Talking(clock.elapsed + 30_000), ui.talk)
        talk(TalkState.Ended(TalkEndReason.LIMIT))
        assertEquals(TalkUi.Idle, ui.talk)
        assertEquals(TalkEnd.LIMIT, ui.talkEnd)
        assertFalse(model.talkActive())
    }

    @Test
    fun aDenialCarriesItsReasonAndRetryAndFadesAfterAWhile() {
        connected()
        talk(TalkState.Denied(TalkDenyReason.Cooldown(12_000u)))
        assertEquals(TalkUi.Denied(TalkDeny.COOLDOWN, clock.elapsed + 12_000), ui.talk)
        talk(TalkState.Denied(null))
        assertEquals(TalkUi.Denied(TalkDeny.UNKNOWN), ui.talk)
        clock.advance(9_000)
        model.onTick(clock.elapsed)
        assertEquals(TalkUi.Idle, ui.talk)
    }

    @Test
    fun theButtonIsUnavailableBeforeThePressWithTheReason() {
        model.onStatus(status(LinkState.CONNECTING), clock.elapsed)
        assertEquals(TalkUi.Unavailable(TalkUnavailable.NOT_CONNECTED), ui.talk)
        connected()
        model.onEvent(peer(talk = TalkStatus.OFF), clock.elapsed)
        assertEquals(TalkUi.Unavailable(TalkUnavailable.OFF), ui.talk)
        model.onEvent(peer(talk = TalkStatus.MUTED), clock.elapsed)
        assertEquals(TalkUi.Unavailable(TalkUnavailable.MUTED), ui.talk)
        model.onEvent(peer(talk = TalkStatus.UNSUPPORTED), clock.elapsed)
        assertEquals(TalkUi.Unavailable(TalkUnavailable.UNSUPPORTED), ui.talk)
        model.onEvent(peer(talker = Talker.OTHER), clock.elapsed)
        assertEquals(TalkUi.Unavailable(TalkUnavailable.OTHER_TALKING), ui.talk)
        model.onEvent(peer(retry = 9_000u), clock.elapsed)
        assertEquals(
            TalkUi.Unavailable(TalkUnavailable.COOLDOWN, clock.elapsed + 9_000),
            ui.talk,
        )
        model.onEvent(peer(), clock.elapsed)
        assertEquals(TalkUi.Idle, ui.talk)
    }

    @Test
    fun theNoisePauseNamesWhoTalks() {
        connected()
        model.onEvent(LinkEvent.NoisePaused(true), clock.elapsed)
        assertEquals(NoisePause.OTHER, ui.noisePause)
        talk(TalkState.Requesting)
        assertEquals(NoisePause.YOU, ui.noisePause)
        talk(TalkState.Ended(TalkEndReason.RELEASED))
        model.onEvent(LinkEvent.NoisePaused(false), clock.elapsed)
        assertNull(ui.noisePause)
    }

    @Test
    fun theUnconfirmedResultAndTheWarningStreakAreShownAndReset() {
        connected()
        model.onEvent(LinkEvent.TalkPlayback(PlaybackResult.UNCONFIRMED), clock.elapsed)
        assertTrue(ui.talkUnconfirmed)
        model.talkPressed()
        assertFalse(ui.talkUnconfirmed)
        model.onEvent(peer(audible = TalkAudible.UNCERTAIN), clock.elapsed)
        assertTrue(ui.talkAudibleUncertain)
    }

    @Test
    fun theListenAfterATalkStartsAndEndsWithTheCoresEvents() {
        connected()
        model.onEvent(LinkEvent.ListenChanged(true), clock.elapsed)
        assertEquals(clock.elapsed + 10_000, ui.afterTalkListenUntilElapsedMs)
        model.onEvent(LinkEvent.ListenChanged(false), clock.elapsed)
        assertNull(ui.afterTalkListenUntilElapsedMs)
    }

    @Test
    fun thePermissionStepIsTrackedAndAPressClearsTheLastResult() {
        connected()
        model.setTalkPermission(TalkPermission.NEEDED)
        assertEquals(TalkPermission.NEEDED, ui.talkPermission)
        talk(TalkState.Ended(TalkEndReason.NO_ANSWER))
        model.talkPressed()
        assertNull(ui.talkEnd)
    }

    @Test
    fun theBabyShowsTheIndicatorAndKeepsTheLastTalkLocally() {
        val baby = BabyModel(clock)
        baby.onEvent(LinkEvent.ParentConnected("Pixel 8"), clock.elapsed)
        baby.onEvent(LinkEvent.TalkActive(true, 0u), clock.elapsed)
        assertTrue(baby.state.value.parentTalking)
        baby.onEvent(LinkEvent.TalkActive(false, 12_000u), clock.elapsed)
        assertFalse(baby.state.value.parentTalking)
        val last = baby.state.value.lastTalk!!
        assertEquals("Pixel 8", last.name)
        assertEquals(12_000, last.durationMs)
    }
}
