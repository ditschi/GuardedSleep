package org.guardedsleep.app

import android.app.Application
import android.os.Looper
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.test.core.app.ApplicationProvider
import java.io.File
import java.time.Duration
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.guardedsleep.app.alerts.AlertController
import org.guardedsleep.app.alerts.NoiseKind
import org.guardedsleep.app.alerts.TonePlayer
import org.guardedsleep.app.alerts.ToneVoice
import org.guardedsleep.app.alerts.ToneVoiceFactory
import org.guardedsleep.app.media.PcmSynth
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.ui.model.AlarmTonePlayback
import org.guardedsleep.app.ui.model.AlarmTones
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.wiring.EntryPoints
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import uniffi.guardedsleep_core.alarmTones
import uniffi.guardedsleep_core.resolveAlarmTone

/** Synthesis, playback and storage of the connection alarm tones (`add-connection-alarm-tone`). */
// sdk 30 like NotificationLanguageTest: the default SDK does not start on this JDK.
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [30])
class AlarmToneTest {
    @get:Rule val tmp = TemporaryFolder()
    private val rate = 16_000

    // The functions this change replaced, kept here to prove that `classic` still sounds the same.
    private fun oldSine(f: Double, ms: Int, amp: Double, fadeMs: Int = 20): ShortArray {
        val n = rate * ms / 1000
        val fade = (rate * fadeMs / 1000).coerceAtLeast(1)
        return ShortArray(n) { i ->
            val env =
                minOf(1.0, i / fade.toDouble(), (n - 1 - i) / fade.toDouble()).coerceAtLeast(0.0)
            (sin(2 * PI * f * i / rate) * amp * env * Short.MAX_VALUE).toInt().toShort()
        }
    }

    @Test
    fun theContractListsTheCoreTonesInPickerOrder() {
        assertEquals(AlarmTones.IDS, alarmTones().map { it.id })
        assertEquals(AlarmTones.DEFAULT, resolveAlarmTone("").id)
        // The UI-side check and the core agree for known and unknown values.
        for (v in AlarmTones.IDS + listOf("", "silent", "Classic", "x".repeat(1024))) {
            assertEquals(v, resolveAlarmTone(v).id, AlarmTones.normalize(v))
        }
        assertEquals(AlarmTones.DEFAULT, AlarmTones.normalize(null))
    }

    @Test
    fun classicRendersTheSameSamplesAsTheOldSynth() {
        val tone = resolveAlarmTone("classic")
        assertArrayEquals(oldSine(660.0, 450, 0.35, 80), PcmSynth.render(tone.soft, rate))
        val gap = ShortArray(rate * 200 / 1000)
        val cycle = oldSine(880.0, 400, 0.8) + gap + oldSine(660.0, 400, 0.8) + gap
        assertArrayEquals(cycle, PcmSynth.render(tone.alarm, rate))
    }

    @Test
    fun everyRecipeRendersItsDurationAndSoundsOutsideTheGaps() {
        for (tone in alarmTones()) {
            val soft = PcmSynth.render(tone.soft, rate)
            assertEquals(tone.id, tone.soft.sumOf { it.ms.toInt() } * rate / 1000, soft.size)
            val alarm = PcmSynth.render(tone.alarm, rate)
            assertEquals(tone.id, tone.alarm.sumOf { it.ms.toInt() } * rate / 1000, alarm.size)
            assertTrue(tone.id, alarm.any { it.toInt() != 0 })
            // The loudest sample of the alarm is much louder than the soft tone's.
            val softPeak = soft.maxOf { abs(it.toInt()) }
            val alarmPeak = alarm.maxOf { abs(it.toInt()) }
            assertTrue("${tone.id} $softPeak $alarmPeak", alarmPeak > softPeak)
        }
    }

    @Test
    fun theReducedGainScalesTheSamples() {
        val tone = resolveAlarmTone("beacon")
        val full = PcmSynth.render(tone.alarm, rate)
        val half = PcmSynth.render(tone.alarm, rate, 0.5)
        assertEquals(full.size, half.size)
        for (i in full.indices) assertEquals((full[i] * 0.5).toInt(), half[i].toInt())
    }

    /** Records what would play; `end()` plays the last sample of the buffer. */
    private class FakeVoices : ToneVoiceFactory {
        class Voice(val samples: ShortArray) : ToneVoice {
            var loop = false
            var started = false
            var stopped = false
            var onEnd: () -> Unit = {}

            override fun start(loop: Boolean, onEnd: () -> Unit) {
                this.loop = loop
                this.onEnd = onEnd
                started = true
            }

            override fun stop() {
                stopped = true
            }

            fun end() = onEnd()
        }

        val made = mutableListOf<Voice>()

        override fun create(samples: ShortArray, rate: Int): ToneVoice =
            Voice(samples).also { made += it }
    }

    private fun peak(samples: ShortArray) = samples.maxOf { abs(it.toInt()) }

    @Test
    fun previewPlaysAtReducedGainReportsItsStateAndStops() {
        val voices = FakeVoices()
        val player = TonePlayer(rate, voices)
        val low = resolveAlarmTone("low")
        player.preview(low)
        assertEquals(AlarmTonePlayback.Preview("low"), player.playback.value)
        val voice = voices.made.single()
        assertTrue(voice.started && !voice.loop)
        // soft + pause + two alarm cycles, at half of the full-gain level.
        val ms = low.soft.sumOf { it.ms.toInt() } + 300 + 2 * low.alarm.sumOf { it.ms.toInt() }
        assertEquals(ms * rate / 1000, voice.samples.size)
        val full = PcmSynth.render(low.alarm, rate)
        assertEquals(peak(full) * 0.5, peak(voice.samples).toDouble(), 2.0)
        player.stopDemo()
        assertTrue(voice.stopped)
        assertEquals(AlarmTonePlayback.Idle, player.playback.value)
    }

    @Test
    fun aPreviewEndsByItself() {
        val voices = FakeVoices()
        val player = TonePlayer(rate, voices)
        player.preview(resolveAlarmTone("beacon"))
        voices.made.single().end()
        assertEquals(AlarmTonePlayback.Idle, player.playback.value)
    }

    @Test
    fun theTestPlaysAtFullGainForAboutEightSeconds() {
        val voices = FakeVoices()
        val player = TonePlayer(rate, voices)
        val tone = resolveAlarmTone("rising")
        player.test(tone)
        assertEquals(AlarmTonePlayback.Test("rising"), player.playback.value)
        val samples = voices.made.single().samples
        assertTrue("${samples.size}", samples.size in 8 * rate..(9.7 * rate).toInt())
        assertEquals(peak(PcmSynth.render(tone.alarm, rate)), peak(samples))
        // Only one at a time: a new preview replaces the test.
        player.preview(resolveAlarmTone("chime"))
        assertTrue(voices.made[0].stopped)
        assertEquals(AlarmTonePlayback.Preview("chime"), player.playback.value)
    }

    @Test
    fun aRealToneStopsAPreviewAndNoPreviewStartsOverARunningAlarm() {
        val voices = FakeVoices()
        val player = TonePlayer(rate, voices)
        player.preview(resolveAlarmTone("rising"))
        player.playSoft(resolveAlarmTone("classic"))
        assertEquals(AlarmTonePlayback.Idle, player.playback.value)
        assertTrue(voices.made[0].stopped)

        player.preview(resolveAlarmTone("rising"))
        player.startAlarm(resolveAlarmTone("classic"))
        assertEquals(AlarmTonePlayback.Idle, player.playback.value)
        val alarm = voices.made.last()
        assertTrue(alarm.loop)
        // While the alarm loops a preview is not started over it.
        val before = voices.made.size
        player.preview(resolveAlarmTone("rising"))
        assertEquals(before, voices.made.size)
        assertEquals(AlarmTonePlayback.Idle, player.playback.value)
        player.stop()
        assertTrue(alarm.stopped)
    }

    @Test
    fun aRunningAlarmSwitchesRecipeAtItsNextCycle() {
        val voices = FakeVoices()
        val player = TonePlayer(rate, voices)
        player.startAlarm(resolveAlarmTone("classic"))
        player.startAlarm(resolveAlarmTone("beacon"))
        // Still the classic cycle until it ends.
        assertEquals(1, voices.made.size)
        assertFalse(voices.made[0].stopped)
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(1_300))
        assertTrue(voices.made[0].stopped)
        assertEquals(2, voices.made.size)
        assertEquals(
            PcmSynth.render(resolveAlarmTone("beacon").alarm, rate).size,
            voices.made[1].samples.size,
        )
        player.stop()
    }

    @Test
    fun aNoiseAlertStopsAPreview() {
        val app: Application = ApplicationProvider.getApplicationContext()
        val entryPoints =
            object : EntryPoints {
                override val mainActivity = android.app.Activity::class.java
                override val alertActivity = android.app.Activity::class.java
                override val alertActionReceiver = android.content.BroadcastReceiver::class.java
            }
        val voices = FakeVoices()
        val alerts =
            AlertController(app, entryPoints, toneId = { "low" }, tones = TonePlayer(rate, voices))
        alerts.previewAlarmTone("low")
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(AlarmTonePlayback.Preview("low"), alerts.alarmTonePlayback.value)
        alerts.noiseAlert(NoiseKind.NEW, probe = false, snoozed = false)
        assertEquals(AlarmTonePlayback.Idle, alerts.alarmTonePlayback.value)
        assertTrue(voices.made.single().stopped)
    }

    @Test
    fun anUnknownStoredToneReadsAsTheDefaultAndAChoiceSurvivesARestart() {
        val dir = tmp.root
        val scope0 = CoroutineScope(Dispatchers.Unconfined)
        val store0 =
            PreferenceDataStoreFactory.create(scope = scope0) { File(dir, "tone.preferences_pb") }
        val repo = SettingsRepository(store0, scope0)
        assertEquals("classic", repo.current.settings.connectionAlarmTone)
        repo.update { it.copy(settings = it.settings.copy(connectionAlarmTone = "chime")) }
        // A restart (a new repository on the same store) reads the stored value back.
        assertEquals(
            "chime",
            SettingsRepository(store0, scope0).current.settings.connectionAlarmTone,
        )

        // A value from a newer or older version (or garbage) is the default, never silence.
        val file = File(dir, "bad.preferences_pb")
        val scope = CoroutineScope(Dispatchers.Unconfined)
        val store =
            PreferenceDataStoreFactory.create(scope = scope) {
                file
            }
        runBlocking {
            store.edit {
                it[stringPreferencesKey("connection_alarm_tone")] = "silent"
            }
            store.data.first()
        }
        val repo2 = SettingsRepository(store, scope)
        assertEquals(
            SettingsState().connectionAlarmTone,
            repo2.current.settings.connectionAlarmTone,
        )
        assertEquals("classic", repo2.current.settings.connectionAlarmTone)
    }
}
