package org.guardedsleep.app

import org.guardedsleep.app.media.AlarmVolume
import org.guardedsleep.app.media.StreamVolume
import org.guardedsleep.app.media.VolumeMemory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.TalkLevel

private class Stream(override val max: Int, var value: Int) : StreamVolume {
    override var fixed = false
    override var index: Int
        get() = value
        set(v) {
            value = v
        }
}

private class Memory : VolumeMemory {
    var saved: Pair<Int, Int>? = null

    override fun save(previous: Int, set: Int) {
        saved = previous to set
    }

    override fun load() = saved

    override fun clear() {
        saved = null
    }
}

/** The talk volume: the stream step nearest the percent, the 70 % cap, a residual track gain. */
class AlarmVolumeTest {
    private fun lvl(pct: Int) = TalkLevel(pct.toUByte())

    private fun prepared(max: Int, now: Int, pct: Int): Triple<Stream, Double, Memory> {
        val stream = Stream(max, now)
        val memory = Memory()
        val gain = AlarmVolume(stream, memory).prepare(lvl(pct))!!
        return Triple(stream, gain, memory)
    }

    @Test
    fun maxSevenMapsEachStepToTheNearestStreamStepWithAResidualGain() {
        // pct -> (stream step, residual gain = requested / (step / max)), cap = round(0.7 * 7) = 5
        val expected = mapOf(10 to 1, 20 to 1, 30 to 2, 40 to 3, 50 to 4, 60 to 4, 70 to 5)
        for ((pct, step) in expected) {
            val (stream, gain, _) = prepared(7, 3, pct)
            assertEquals("step of $pct %", step, stream.value)
            val requested = minOf(pct / 100.0, 5.0 / 7.0)
            assertEquals("gain of $pct %", minOf(1.0, requested / (step / 7.0)), gain, 1e-9)
            assertTrue(gain <= 1.0)
        }
    }

    @Test
    fun maxFifteenMapsEachStepAndNeighbouringPercentsStillDiffer() {
        val steps = (10..70 step 10).map { prepared(15, 1, it).first.value }
        // 1.5 -> 2, 3, 4.5 -> 5, 6, 7.5 -> 8, 9, 10.5 -> cap 11
        assertEquals(listOf(2, 3, 5, 6, 8, 9, 11), steps)
        // Percents that land on the same stream step still differ through the track gain.
        val gains = listOf(40, 50).map { prepared(15, 1, it).second }
        assertEquals(6, prepared(15, 1, 40).first.value)
        assertTrue(gains[0] != gains[1])
    }

    @Test
    fun theTopNeverExceedsSeventyPercentOfTheMaximumByMoreThanTheRoundingOfTheCap() {
        for (max in listOf(5, 7, 10, 15, 30)) {
            val (stream, gain, _) = prepared(max, 1, 70)
            assertTrue(stream.value <= Math.round(0.7 * max).toInt())
            // The requested level is the cap itself: 70 % of the maximum, never more.
            val played = gain * stream.value / max
            assertTrue("max $max played $played", played <= 0.7 + 1e-9)
        }
    }

    @Test
    fun theStreamIsLoweredOrRaisedToTheStepAndRestoredOnlyWhenUnchanged() {
        val stream = Stream(15, 12)
        val memory = Memory()
        val volume = AlarmVolume(stream, memory)
        volume.prepare(lvl(30))
        assertEquals(5, stream.value)
        assertEquals(12 to 5, memory.saved)
        volume.restore()
        assertEquals(12, stream.value)
        assertNull(memory.saved)

        stream.value = 2
        volume.prepare(lvl(30))
        assertEquals(5, stream.value)
        stream.value = 9 // the user turned it meanwhile
        volume.restore()
        assertEquals(9, stream.value)
    }

    @Test
    fun aStreamAlreadyAtTheStepIsLeftAloneButStillGetsTheResidualGain() {
        val stream = Stream(15, 8)
        val memory = Memory()
        val gain = AlarmVolume(stream, memory).prepare(lvl(50))!!
        assertEquals(8, stream.value)
        assertNull(memory.saved)
        assertEquals(0.5 / (8.0 / 15.0), gain, 1e-9)
    }
}
