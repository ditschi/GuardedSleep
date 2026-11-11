package org.guardedsleep.app

import java.io.File
import org.guardedsleep.app.config.TalkPolicy
import org.guardedsleep.app.media.AlarmVolume
import org.guardedsleep.app.media.ChimeSynth
import org.guardedsleep.app.media.StreamVolume
import org.guardedsleep.app.media.TalkPlayout
import org.guardedsleep.app.media.TalkSpeaker
import org.guardedsleep.app.media.VolumeMemory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.TalkLevel

private class FakeStream(override val max: Int = 15, var value: Int = 3) : StreamVolume {
    override var fixed = false
    var refuse = false
    override var index: Int
        get() = value
        set(v) {
            if (refuse) throw SecurityException("dnd")
            value = v
        }
}

private class FakeMemory : VolumeMemory {
    var saved: Pair<Int, Int>? = null

    override fun save(previous: Int, set: Int) {
        saved = previous to set
    }

    override fun load() = saved

    override fun clear() {
        saved = null
    }
}

private class FakeSpeaker : TalkSpeaker {
    val enabled = HashSet<UInt>()
    var muted = true
    var gain = 0.0
    var chimes = 0
    var chimeDone: (() -> Unit)? = null

    override fun setReceiverEnabled(attempt: UInt, enabled: Boolean) {
        if (enabled) this.enabled += attempt else this.enabled -= attempt
    }

    override fun setReceiverGain(attempt: UInt, gain: Double) {
        this.gain = gain
    }

    override fun setSpeakerMuted(muted: Boolean) {
        this.muted = muted
    }

    override fun playChime(gain: Double, onDone: () -> Unit) {
        chimes++
        chimeDone = onDone
    }

    override fun stopChime() {
        chimeDone = null
    }
}

class TalkPlayoutTest {
    private val stream = FakeStream()
    private val memory = FakeMemory()
    private val speaker = FakeSpeaker()
    private val chimeDone = mutableListOf<UInt>()
    private val muted = mutableListOf<Boolean>()
    private val outside = mutableListOf<UInt>()
    private val volume = AlarmVolume(stream, memory)
    private val playout =
        TalkPlayout(speaker, volume, { chimeDone += it }, { muted += it }, { outside += it })

    @Test
    fun playbackExistsOnlyInsideTheLeaseAndTheDeadlineMutesWithoutAStop() {
        playout.start()
        assertTrue(speaker.muted)
        playout.open(1u, TalkLevel.LOW, 800, untilMs = 10_000, nowMs = 1_000)
        assertFalse(speaker.muted)
        assertEquals(setOf(1u), speaker.enabled)
        playout.tick(9_999)
        assertFalse(speaker.muted)
        playout.tick(10_000)
        assertTrue(speaker.muted)
        assertTrue(speaker.enabled.isEmpty())
        assertEquals(3, stream.value) // restored: the user's value, unchanged meanwhile
    }

    @Test
    fun onlyTheLeasedAttemptsReceiverIsEnabledAndALaterLeaseReplacesIt() {
        playout.open(1u, TalkLevel.LOW, 800, 10_000, 0)
        playout.open(2u, TalkLevel.LOW, 800, 10_000, 100)
        assertEquals(setOf(2u), speaker.enabled)
        playout.close(200)
        assertTrue(speaker.enabled.isEmpty())
    }

    @Test
    fun theVoiceFadesInOverTheFadeTime() {
        playout.open(1u, TalkLevel.HIGH, 800, 60_000, 0)
        playout.tick(0)
        assertEquals(0.0, speaker.gain, 1e-9)
        playout.tick(400)
        assertTrue(speaker.gain in 0.4..0.6)
        playout.tick(800)
        assertEquals(1.0, speaker.gain, 1e-9)
    }

    @Test
    fun aFullScaleStepIsCappedAtTheCeilingOnTheNextSample() {
        playout.open(1u, TalkLevel.HIGH, 1, 60_000, 0)
        playout.tick(100)
        playout.onTalkLevel(1.0, 200)
        assertTrue(speaker.gain * 1.0 <= TalkPolicy.CEILING_RMS + 1e-9)
        // It gives the gain back slowly, never above the target.
        repeat(100) { playout.onTalkLevel(0.01, 300L + it * 100) }
        assertTrue(speaker.gain <= 1.0)
    }

    @Test
    fun theChimeReportsItsEndAndAChimeThatCannotPlayEndsAtOnce() {
        playout.playChime(4u, TalkLevel.MEDIUM)
        assertEquals(1, speaker.chimes)
        speaker.chimeDone!!.invoke()
        assertEquals(listOf(4u), chimeDone)
        stream.fixed = true
        playout.close(0)
        playout.playChime(5u, TalkLevel.MEDIUM)
        assertEquals(listOf(4u, 5u), chimeDone)
        assertEquals(true, muted.last())
    }

    @Test
    fun anUnreachableLevelReportsMutedAndNeverOpens() {
        stream.refuse = true
        stream.value = 1
        playout.open(1u, TalkLevel.HIGH, 800, 10_000, 0)
        assertTrue(speaker.enabled.isEmpty())
        assertEquals(true, muted.last())
    }

    @Test
    fun talkPacketsOutsideALeaseAreReportedOncePerAttempt() {
        playout.onTalkPackets(1u, 100, 0)
        playout.onTalkPackets(1u, 103, 2_000)
        assertTrue(outside.isEmpty())
        playout.onTalkPackets(1u, 200, 3_000)
        playout.onTalkPackets(1u, 400, 4_000)
        assertEquals(listOf(1u), outside)
    }

    @Test
    fun packetsOfTheLeaseAndItsTailAreNotAViolation() {
        playout.onTalkPackets(1u, 0, 0)
        playout.open(1u, TalkLevel.LOW, 800, 10_000, 100)
        playout.onTalkPackets(1u, 500, 5_000)
        playout.close(6_000)
        playout.onTalkPackets(1u, 520, 6_500)
        playout.onTalkPackets(1u, 520, 9_000)
        assertTrue(outside.isEmpty())
    }

    @Test
    fun theVolumeIsRaisedOnlyAsFarAsNeededAndRestoredOnlyIfUnchanged() {
        stream.value = 1
        assertEquals(1.0, volume.prepare(TalkLevel.LOW)!!, 1e-9)
        assertEquals(4, stream.value) // ceil(0.25 * 15)
        assertEquals(1 to 4, memory.saved)
        volume.restore()
        assertEquals(1, stream.value)
        assertNull(memory.saved)

        stream.value = 1
        volume.prepare(TalkLevel.LOW)
        stream.value = 9 // the user changed it meanwhile
        volume.restore()
        assertEquals(9, stream.value)
    }

    @Test
    fun aLoudStreamIsAttenuatedPerTrackInsteadOfLowered() {
        stream.value = 12
        val g = volume.prepare(TalkLevel.LOW)!!
        assertEquals(12, stream.value)
        assertEquals(4.0 / 12.0, g, 1e-9)
    }

    @Test
    fun highStaysAtOrBelowAboutSeventyPercentOfTheMaximum() {
        stream.value = 0
        volume.prepare(TalkLevel.HIGH)
        assertTrue(stream.value <= Math.ceil(0.7 * stream.max).toInt())
    }

    @Test
    fun aRaiseLeftByACrashIsUndoneAtTheNextStartUnderTheSameRule() {
        memory.saved = 2 to 6
        stream.value = 6
        playout.start()
        assertEquals(2, stream.value)
        memory.saved = 2 to 6
        stream.value = 8
        playout.start()
        assertEquals(8, stream.value)
        assertNull(memory.saved)
    }

    @Test
    fun theChimeIsSoftBoundedAndNeverBelowTheAudibleFloor() {
        val pcm = ChimeSynth.generate(1.0)
        assertEquals(ChimeSynth.RATE * TalkPolicy.CHIME_MS / 1000, pcm.size)
        val peak = pcm.maxOf { kotlin.math.abs(it.toInt()) } / 32767.0
        assertTrue(peak in 0.25..TalkPolicy.CHIME_PEAK + 0.01)
        assertEquals(0, pcm.first().toInt())
        assertTrue(kotlin.math.abs(pcm.last().toInt()) < 400)
        val quiet = ChimeSynth.generate(0.01)
        assertTrue(
            quiet.maxOf { kotlin.math.abs(it.toInt()) } / 32767.0 >= TalkPolicy.CHIME_FLOOR - 0.01
        )
    }

    @Test
    fun noTalkPathReadsOrRecordsAudio() {
        val dir = File("src/main/kotlin/org/guardedsleep/app/media")
        assertTrue(dir.isDirectory)
        val forbidden =
            Regex("""MediaRecorder|AudioRecord\(|startAecDump|RtcEventLog|AudioRecord\.""")
        val files = dir.walkTopDown().filter { it.extension == "kt" }.toList()
        for (f in files) {
            val code =
                f.readText()
                    .lines()
                    .filterNot {
                        it.trimStart().startsWith("*") ||
                            it.trimStart().startsWith("//") ||
                            it.trimStart().startsWith("/*")
                    }
                    .joinToString("\n")
            assertFalse("${f.name} uses a recording API", forbidden.containsMatchIn(code))
        }
        val peer = File(dir, "libwebrtc/LibWebRtcAttemptPeer.kt").readText()
        assertFalse("the attempt peer attaches no sink", peer.contains("addSink"))
        val engine = File(dir, "libwebrtc/LibWebRtcMediaEngine.kt").readText()
        // The one sink measures the parent's monitoring audio in `onRemoteAudio`; the talk line
        // (a baby receiver, a parent sender) never reaches it.
        assertEquals(1, Regex("""\.addSink\(""").findAll(engine).count())
        assertTrue(engine.contains("if (role == Role.BABY) builder.setAudioBufferCallback"))
        assertFalse(engine.contains("talkTrack?.addSink") || engine.contains("talkTrack.addSink"))
    }
}
