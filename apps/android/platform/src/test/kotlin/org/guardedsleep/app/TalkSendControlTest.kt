package org.guardedsleep.app

import org.guardedsleep.app.media.TalkSendControl
import org.guardedsleep.app.media.TalkSendPort
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TalkSendControlTest {
    private class FakePort(var trackOk: Boolean = true, var activeOk: Boolean = true) :
        TalkSendPort<String> {
        val calls = mutableListOf<String>()
        var track: String? = null
        var active = true

        override fun setTrack(track: String?): Boolean {
            calls += "track:$track"
            if (!trackOk) return false
            this.track = track
            return true
        }

        override fun setActive(active: Boolean): Boolean {
            calls += "active:$active"
            if (!activeOk) return false
            this.active = active
            return true
        }
    }

    @Test
    fun theLineStartsIdleWithNoTrack() {
        val port = FakePort()
        assertTrue(TalkSendControl(port).idle())
        assertFalse(port.active)
        assertEquals(null, port.track)
    }

    @Test
    fun theTrackIsOnTheSenderBeforeTheLineTurnsActive() {
        val port = FakePort()
        val control = TalkSendControl(port)
        control.idle()
        port.calls.clear()
        assertTrue(control.attach("mic"))
        assertEquals(listOf("track:mic", "active:true"), port.calls)
    }

    @Test
    fun theLineTurnsInactiveBeforeTheTrackLeaves() {
        val port = FakePort()
        val control = TalkSendControl(port)
        control.attach("mic")
        port.calls.clear()
        assertTrue(control.detach())
        assertEquals(listOf("active:false", "track:null"), port.calls)
        assertFalse(port.active)
    }

    @Test
    fun anActivationThatFailsLeavesTheLineIdle() {
        val port = FakePort()
        val control = TalkSendControl(port)
        control.idle()
        port.activeOk = false
        assertFalse(control.attach("mic"))
        assertEquals(null, port.track)
        assertFalse(port.active)
    }

    @Test
    fun aTrackThatCannotBeSetNeverActivatesTheLine() {
        val port = FakePort(trackOk = false)
        val control = TalkSendControl(port)
        control.idle()
        port.calls.clear()
        assertFalse(control.attach("mic"))
        assertEquals(listOf("track:mic"), port.calls)
    }

    @Test
    fun detachingTwiceStaysIdle() {
        val port = FakePort()
        val control = TalkSendControl(port)
        control.attach("mic")
        control.detach()
        assertTrue(control.detach())
        assertFalse(port.active)
    }
}
