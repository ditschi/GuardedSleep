package org.guardedsleep.app.readiness

import android.Manifest
import android.annotation.SuppressLint
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import java.net.URI
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.sqrt
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.guardedsleep.app.ui.model.DeviceCheck
import org.guardedsleep.app.ui.model.DeviceCheckReason
import org.guardedsleep.app.ui.model.MailboxReach

/** Facts about the baby phone, separated from the rules so the rules are testable on the JVM. */
interface DeviceCheckEnv {
    fun micGranted(): Boolean

    fun batteryPct(): Int?

    fun charging(): Boolean

    fun batteryUnhealthy(): Boolean

    fun batteryOptimizationIgnored(): Boolean

    fun dndAccessGranted(): Boolean

    /**
     * The microphone's RMS level (0..1) over about [ms] milliseconds; null when it cannot record or
     * [cancelled] turned true (checked between reads, so the microphone is released at once).
     */
    fun sampleMicRms(ms: Long, cancelled: () -> Boolean): Double?

    /**
     * True once a connect to [url] opens within [timeoutMs]. No frame is sent: no room id, tag or
     * envelope leaves the phone.
     */
    fun probeMailbox(url: String, timeoutMs: Long): Boolean
}

/** What the check measured; a null fact was skipped and adds no reason. */
data class DeviceFacts(
    val micGranted: Boolean,
    val micRms: Double?,
    val batteryPct: Int?,
    val charging: Boolean,
    val batteryUnhealthy: Boolean,
    val batteryOptimizationIgnored: Boolean,
    val dndAccessGranted: Boolean,
    val mailboxes: List<MailboxReach>,
)

data class DeviceCheckResult(
    val check: DeviceCheck,
    /** Worst first. */
    val reasons: List<DeviceCheckReason>,
    val mailboxes: List<MailboxReach>,
)

/** The rules: pure, so every reason is tested without a device. */
object DeviceCheckRules {
    const val LOW_BATTERY_PCT = 20

    /** Below this level the microphone delivers digital silence (muted or privacy toggle). */
    const val SILENT_RMS = 0.0002

    fun evaluate(f: DeviceFacts): DeviceCheckResult {
        val bad = mutableListOf<DeviceCheckReason>()
        val limited = mutableListOf<DeviceCheckReason>()
        if (!f.micGranted) bad += DeviceCheckReason.MIC_PERMISSION_MISSING
        else if (f.micRms != null && f.micRms < SILENT_RMS) bad += DeviceCheckReason.MIC_SILENT
        if (f.batteryUnhealthy) bad += DeviceCheckReason.BATTERY_UNHEALTHY
        val pct = f.batteryPct
        val lowAndUnplugged = pct != null && pct < LOW_BATTERY_PCT && !f.charging
        if (lowAndUnplugged) bad += DeviceCheckReason.BATTERY_LOW_NOT_CHARGING
        if (f.mailboxes.isNotEmpty() && f.mailboxes.none { it.reachable })
            bad += DeviceCheckReason.NO_MAILBOX_REACHABLE
        if (!f.charging && !lowAndUnplugged) limited += DeviceCheckReason.NOT_CHARGING
        if (!f.batteryOptimizationIgnored) limited += DeviceCheckReason.BATTERY_OPTIMIZED
        if (!f.dndAccessGranted) limited += DeviceCheckReason.DND_ACCESS_MISSING
        if (f.mailboxes.any { it.reachable } && f.mailboxes.any { !it.reachable })
            limited += DeviceCheckReason.SOME_MAILBOXES_UNREACHABLE
        val check =
            when {
                bad.isNotEmpty() -> DeviceCheck.NOT_RECOMMENDED
                limited.isNotEmpty() -> DeviceCheck.LIMITED
                else -> DeviceCheck.OK
            }
        return DeviceCheckResult(check, bad + limited, f.mailboxes)
    }
}

/**
 * The baby device check (`device-setup`, "Baby device check"). Blocking: call it off the main
 * thread. The mailbox probes run in parallel, so the worst case is the probe timeout plus the mic
 * sample at most.
 */
class DeviceChecker(private val env: DeviceCheckEnv) {
    private val pool: ExecutorService = Executors.newCachedThreadPool { r ->
        Thread(r, "gs-device-probe").apply { isDaemon = true }
    }

    /**
     * [mailboxUrls] are the relay URLs to probe; [sampleMic] is false while a run holds the
     * microphone.
     */
    fun check(
        mailboxUrls: List<String>,
        sampleMic: Boolean,
        cancelled: () -> Boolean = { false },
    ): DeviceCheckResult {
        val probes =
            mailboxUrls.distinct().map { url ->
                url to
                    pool.submit<Boolean> {
                        runCatching { env.probeMailbox(url, PROBE_TIMEOUT_MS) }.getOrDefault(false)
                    }
            }
        val granted = env.micGranted()
        val rms =
            if (granted && sampleMic)
                runCatching { env.sampleMicRms(MIC_SAMPLE_MS, cancelled) }.getOrNull()
            else null
        val reach = probes.map { (url, f) ->
            val ok = runCatching {
                f.get(PROBE_TIMEOUT_MS + 2_000, TimeUnit.MILLISECONDS)
            }
                .getOrDefault(false)
            MailboxReach(hostOf(url), ok)
        }
        return DeviceCheckRules.evaluate(
            DeviceFacts(
                micGranted = granted,
                micRms = rms,
                batteryPct = env.batteryPct(),
                charging = env.charging(),
                batteryUnhealthy = env.batteryUnhealthy(),
                batteryOptimizationIgnored = env.batteryOptimizationIgnored(),
                dndAccessGranted = env.dndAccessGranted(),
                mailboxes = reach,
            )
        )
    }

    companion object {
        const val PROBE_TIMEOUT_MS = 10_000L
        const val MIC_SAMPLE_MS = 1_500L

        /** The host only: no path, query or userinfo reaches the UI. */
        internal fun hostOf(url: String): String =
            runCatching { URI(url).host }.getOrNull()?.takeIf { it.isNotBlank() } ?: "?"
    }
}

class AndroidDeviceCheckEnv(private val context: Context) : DeviceCheckEnv {
    private val battery = BatteryReader(context)
    private val probeClient: OkHttpClient =
        OkHttpClient.Builder()
            .connectTimeout(DeviceChecker.PROBE_TIMEOUT_MS, TimeUnit.MILLISECONDS)
            .build()

    override fun micGranted(): Boolean =
        context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED

    override fun batteryPct(): Int? = battery.pct()

    override fun charging(): Boolean = battery.charging()

    override fun batteryUnhealthy(): Boolean = battery.unhealthy()

    override fun batteryOptimizationIgnored(): Boolean = battery.optimizationIgnored()

    override fun dndAccessGranted(): Boolean =
        context
            .getSystemService(NotificationManager::class.java)
            ?.isNotificationPolicyAccessGranted == true

    @SuppressLint("MissingPermission") // micGranted() is checked by the caller.
    override fun sampleMicRms(ms: Long, cancelled: () -> Boolean): Double? {
        val rate = SAMPLE_RATE
        val minBuf =
            AudioRecord.getMinBufferSize(
                rate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
            )
        if (minBuf <= 0) return null
        val rec =
            try {
                AudioRecord(
                    MediaRecorder.AudioSource.MIC,
                    rate,
                    AudioFormat.CHANNEL_IN_MONO,
                    AudioFormat.ENCODING_PCM_16BIT,
                    minBuf * 2,
                )
            } catch (_: Exception) {
                return null
            }
        try {
            if (rec.state != AudioRecord.STATE_INITIALIZED) return null
            rec.startRecording()
            if (rec.recordingState != AudioRecord.RECORDSTATE_RECORDING) return null
            val buf = ShortArray(rate / 10)
            // The first 300 ms are warm-up; the level counts the rest.
            val warmUpSamples = rate * 3 / 10
            val total = (rate * ms / 1000).toInt() + warmUpSamples
            var read = 0
            var counted = 0L
            var sumSq = 0.0
            while (read < total) {
                // A run that starts meanwhile needs the microphone: stop at once.
                if (cancelled()) return null
                val n = rec.read(buf, 0, buf.size)
                if (n <= 0) return null
                for (i in 0 until n) {
                    if (read + i >= warmUpSamples) {
                        val v = buf[i] / 32768.0
                        sumSq += v * v
                        counted++
                    }
                }
                read += n
            }
            return if (counted == 0L) null else sqrt(sumSq / counted)
        } catch (_: Exception) {
            return null
        } finally {
            runCatching { rec.stop() }
            rec.release()
        }
    }

    override fun probeMailbox(url: String, timeoutMs: Long): Boolean {
        val request = runCatching { Request.Builder().url(url).build() }.getOrNull() ?: return false
        val opened = CountDownLatch(1)
        val ok = AtomicBoolean(false)
        val socket =
            probeClient.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: Response) {
                        ok.set(true)
                        opened.countDown()
                        webSocket.close(1000, null)
                    }

                    override fun onFailure(
                        webSocket: WebSocket,
                        t: Throwable,
                        response: Response?,
                    ) = opened.countDown()
                },
            )
        val done = opened.await(timeoutMs, TimeUnit.MILLISECONDS)
        if (!done) socket.cancel()
        return done && ok.get()
    }

    private companion object {
        const val SAMPLE_RATE = 16_000
    }
}
