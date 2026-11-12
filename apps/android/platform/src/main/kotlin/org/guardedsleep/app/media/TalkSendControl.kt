package org.guardedsleep.app.media

/** The parent's talk sender as the control needs it (a thin adapter over the libwebrtc sender). */
interface TalkSendPort<T> {
    /** Puts [track] on the sender, or takes it off with null. */
    fun setTrack(track: T?): Boolean

    /** Switches the sender's single encoding on or off (`RtpEncodingParameters.active`). */
    fun setActive(active: Boolean): Boolean
}

/**
 * Keeps the parent's talk line silent outside a talk. libwebrtc starts a send stream, and with it
 * the audio device module's recording, for every negotiated `sendonly` line, track or not: the
 * emulator gate saw 50 packets per second and an open `AudioRecord` from the connect on, with no
 * track on the sender. The only switch that stops the stream is the encoding's `active` flag, so
 * the line is idle (inactive, no track) from the negotiation until [attach] and again after
 * [detach].
 *
 * Order matters: the track is on the sender before the line turns active (the first frame is voice)
 * and the line turns inactive before the track leaves (no frame without an owner).
 */
class TalkSendControl<T>(private val port: TalkSendPort<T>) {
    /** Idle state at negotiation time; false when the sender refused (the caller reports it). */
    fun idle(): Boolean = port.setActive(false) && port.setTrack(null)

    /** Track on, line active. On any failure the line is left idle and false is returned. */
    fun attach(track: T): Boolean {
        if (!port.setTrack(track)) return false
        if (port.setActive(true)) return true
        detach()
        return false
    }

    /** Line inactive, then track off; true when both steps worked. */
    fun detach(): Boolean {
        val off = port.setActive(false)
        val cleared = port.setTrack(null)
        return off && cleared
    }
}
