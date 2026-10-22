package org.guardedsleep.app.media

import java.nio.ByteBuffer

/**
 * The microphone PCM that is about to be encoded: (a) mixes the debug cry injection, (b) measures
 * the level for the core, (c) mixes the probe tone after measuring so the tone cannot trigger the
 * gate. Vendor-free so it is unit-testable; the engine adapts its audio-buffer callback to it.
 */
class CaptureTap(private val micWindow: RmsWindow, private val probe: ProbeTone) {
    /**
     * Returns [captureTimeNs] unchanged: libwebrtc replaces its capture timestamp with the result.
     */
    fun onBuffer(
        buffer: ByteBuffer,
        channels: Int,
        rate: Int,
        bytesRead: Int,
        captureTimeNs: Long,
    ): Long {
        // The valid PCM is [0, bytesRead); the helpers read position..limit of a view.
        val pcm: ByteBuffer = buffer.duplicate().clear().limit(bytesRead) as ByteBuffer
        InjectedAudio.mix(pcm, channels)
        val (sum, n) = Pcm.sumSquares(pcm)
        micWindow.add(sum, n)
        probe.mix(pcm, channels, rate)
        return captureTimeNs
    }
}
