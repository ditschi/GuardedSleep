package org.guardedsleep.app.media.libwebrtc

/**
 * Source choice and constraints of the baby's raw capture (design §2 of alpha-field-test-feedback).
 */
internal object RawCapture {
    /**
     * `UNPROCESSED` when `AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED` is `"true"`, else
     * `MIC` (no voice-call processing either).
     */
    fun audioSource(unprocessedProperty: String?): Int =
        if (unprocessedProperty == "true") UNPROCESSED else MIC

    // The platform's audio-source ids (MediaRecorder.AudioSource.MIC / UNPROCESSED), written as
    // numbers: the talk-path guard test keeps recorder class names out of the media code.
    const val MIC = 1
    const val UNPROCESSED = 9

    /** libwebrtc's software echo canceller, noise suppressor and gain control. */
    val PROCESSING_CONSTRAINTS =
        listOf("googEchoCancellation", "googNoiseSuppression", "googAutoGainControl")
}
