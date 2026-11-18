package org.guardedsleep.app.ui

import org.guardedsleep.app.ui.common.Headline
import org.guardedsleep.app.ui.common.headlineOf
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.ParentMonitorState
import org.guardedsleep.app.ui.model.PeerStatusUi
import org.guardedsleep.app.ui.model.SoundEventUi
import org.junit.Assert.assertEquals
import org.junit.Test

/** The hero line, one case per branch and the order between them (`audio-monitoring`). */
class HeadlineTest {
    private fun connected(p: ParentMonitorState = ParentMonitorState()) = p.copy(link = Connected)

    private fun head(p: ParentMonitorState) = headlineOf(p, ELAPSED, WALL)

    @Test
    fun notConnectedNeverSaysQuietAndNamesTheLastContact() {
        val down =
            ParentMonitorState(
                link = link(LinkPhase.RECONNECTING, 2, 60_000),
                quietSinceWallMs = WALL - 600_000,
            )
        assertEquals(Headline.LastHeard(WALL - 60_000), head(down))
    }

    @Test
    fun nothingHeardYet() {
        assertEquals(
            Headline.NoContact,
            head(ParentMonitorState(link = link(LinkPhase.CONNECTING))),
        )
    }

    @Test
    fun aSilentMicrophoneBeatsEverythingWhileConnected() {
        val p =
            connected(
                ParentMonitorState(
                    peer = PeerStatusUi(micSilent = true),
                    quietSinceWallMs = WALL - 60_000,
                    noiseAlertActive = true,
                    noiseSinceWallMs = WALL - 5_000,
                )
            )
        assertEquals(Headline.CantHear, head(p))
    }

    @Test
    fun aRunningSoundIsPausedWhileNoiseAlertsAre() {
        val running = ParentMonitorState(noiseAlertActive = true, noiseSinceWallMs = WALL - 5_000)
        assertEquals(Headline.SoundSince(WALL - 5_000), head(connected(running)))
        assertEquals(
            Headline.SoundPaused(WALL - 5_000),
            head(connected(running.copy(snoozedUntilWallMs = WALL + 60_000))),
        )
        // A pause that has ended no longer counts.
        assertEquals(
            Headline.SoundSince(WALL - 5_000),
            head(connected(running.copy(snoozedUntilWallMs = WALL))),
        )
    }

    @Test
    fun aRunningSoundWithoutAStartUsesNow() {
        assertEquals(
            Headline.SoundSince(WALL),
            head(connected(ParentMonitorState(noiseAlertActive = true))),
        )
    }

    @Test
    fun quietSinceBeatsTheLastSound() {
        val p =
            ParentMonitorState(
                quietSinceWallMs = WALL - 60_000,
                lastSound = SoundEventUi(WALL - 90_000, 30_000),
            )
        assertEquals(Headline.AllQuiet(WALL - 60_000), head(connected(p)))
    }

    @Test
    fun theLastSoundWhileQuietIsNotEarned() {
        val p = ParentMonitorState(lastSound = SoundEventUi(WALL - 90_000, 30_000))
        assertEquals(Headline.SoundAt(WALL - 90_000, 30_000), head(connected(p)))
    }

    @Test
    fun plainMonitoringOtherwise() {
        assertEquals(Headline.Monitoring, head(connected()))
    }
}
