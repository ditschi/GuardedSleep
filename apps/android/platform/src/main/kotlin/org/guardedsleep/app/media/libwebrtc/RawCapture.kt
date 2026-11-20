package org.guardedsleep.app.media.libwebrtc

import android.media.MediaRecorder

/**
 * Source choice and constraints of the baby's raw capture (design §2 of alpha-field-test-feedback).
 */
internal object RawCapture {
    /**
     * `UNPROCESSED` when `AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED` is `"true"`, else
     * `MIC` (no voice-call processing either).
     */
    fun audioSource(unprocessedProperty: String?): Int =
        if (unprocessedProperty == "true") MediaRecorder.AudioSource.UNPROCESSED
        else MediaRecorder.AudioSource.MIC

    /** libwebrtc's software echo canceller, noise suppressor and gain control. */
    val PROCESSING_CONSTRAINTS =
        listOf("googEchoCancellation", "googNoiseSuppression", "googAutoGainControl")
}
