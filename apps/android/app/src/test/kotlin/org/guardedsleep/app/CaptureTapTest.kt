package org.guardedsleep.app

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.guardedsleep.app.media.CaptureTap
import org.guardedsleep.app.media.ProbeTone
import org.guardedsleep.app.media.RmsWindow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class CaptureTapTest {
    @Test
    fun returnsTheIncomingCaptureTimestampUnchanged() {
        val window = RmsWindow()
        val tap = CaptureTap(window, ProbeTone())
        val pcm = ByteBuffer.allocate(960).order(ByteOrder.LITTLE_ENDIAN)
        repeat(480) { pcm.putShort(1000) }
        assertEquals(123_456_789L, tap.onBuffer(pcm, 1, 48_000, 960, 123_456_789L))
        assertEquals(0L, tap.onBuffer(pcm, 1, 48_000, 960, 0L))
        assertNotNull(window.drain())
    }
}
