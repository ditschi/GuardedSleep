package org.guardedsleep.app

import android.media.MediaRecorder
import org.guardedsleep.app.media.libwebrtc.RawCapture
import org.junit.Assert.assertEquals
import org.junit.Test

class RawCaptureTest {
    @Test
    fun unprocessedWhenSupported() {
        assertEquals(MediaRecorder.AudioSource.UNPROCESSED, RawCapture.audioSource("true"))
    }

    @Test
    fun micWhenNotSupported() {
        assertEquals(MediaRecorder.AudioSource.MIC, RawCapture.audioSource("false"))
        assertEquals(MediaRecorder.AudioSource.MIC, RawCapture.audioSource(null))
    }

    @Test
    fun allSoftwareProcessorsListed() {
        assertEquals(3, RawCapture.PROCESSING_CONSTRAINTS.size)
    }
}
