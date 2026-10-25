package org.guardedsleep.core

import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.Action
import uniffi.guardedsleep_core.CoreLink
import uniffi.guardedsleep_core.CoreSession
import uniffi.guardedsleep_core.Input
import uniffi.guardedsleep_core.InviteMailbox
import uniffi.guardedsleep_core.LinkAction
import uniffi.guardedsleep_core.LinkConfig
import uniffi.guardedsleep_core.LinkInput
import uniffi.guardedsleep_core.MailboxKind
import uniffi.guardedsleep_core.ParentSecret
import uniffi.guardedsleep_core.Role
import uniffi.guardedsleep_core.SessionEvent
import uniffi.guardedsleep_core.conformanceEval
import uniffi.guardedsleep_core.defaultSessionConfig

/**
 * AC-5 (ADR-015): the Kotlin binding must reproduce the golden vectors that the Rust and WASM
 * suites check, using the host build of the core loaded through JNA.
 */
class ConformanceTest {
    private val vectors = JSONArray(File(System.getProperty("conformance.vectors")!!).readText())

    @Test
    fun allVectorsMatchThroughUniffi() {
        assertTrue(vectors.length() > 0)
        for (i in 0 until vectors.length()) {
            val case = vectors.getJSONObject(i)
            val expect = case.get("expect")
            val input = JSONObject(case.toString()).apply { remove("expect") }
            val got = parseJson(conformanceEval(input.toString()))
            assertJsonEquals(case.getString("name"), expect, got)
        }
    }

    @Test
    fun committedTracesReplayThroughUniffi() {
        val files =
            File(System.getProperty("conformance.traces")!!)
                .listFiles { f -> f.extension == "json" }!!
                .sortedBy { it.name }
        assertTrue(files.isNotEmpty())
        for (f in files) {
            val trace = JSONObject(f.readText())
            val got = parseJson(conformanceEval(trace.toString()))
            assertJsonEquals(f.name, trace.get("expect"), got)
        }
    }

    /**
     * Typed replay: the trace inputs are lowered as generated UniFFI types into [CoreLink], and the
     * lifted actions are compared with the committed expectation (action and event kind, attempt,
     * message text, SDP). This exercises the record/enum lift and lower of the 17 remote types.
     */
    @Test
    fun linkTracesReplayThroughTypedApi() {
        val files =
            File(System.getProperty("conformance.traces")!!)
                .listFiles { f -> f.extension == "json" }!!
                .sortedBy { it.name }
        assertTrue(files.isNotEmpty())
        for (f in files) {
            val trace = JSONObject(f.readText())
            val c = trace.getJSONObject("config")
            val link =
                CoreLink(
                    LinkConfig(
                        role = if (c.getString("role") == "baby") Role.BABY else Role.PARENT,
                        pskB64url = c.getString("psk_b64url"),
                        seed =
                            c.getString("seed")
                                .chunked(2)
                                .map { it.toInt(16).toByte() }
                                .toByteArray(),
                        pairedBefore = c.getBoolean("paired_before"),
                        heartbeatIntervalMs = c.getLong("heartbeat_interval_ms").toULong(),
                        alarmAfterMs = c.getLong("alarm_after_ms").toULong(),
                        noiseThreshold = c.getDouble("noise_threshold"),
                        noiseHoldMs = c.getLong("noise_hold_ms").toULong(),
                        requireDtlsFingerprint = c.getBoolean("require_dtls_fingerprint"),
                        pinnedParent =
                            if (c.isNull("pinned_parent")) null
                            else ParentSecret(hexBytes(c.getString("pinned_parent"))),
                        deviceName = c.optString("device_name", ""),
                        mailboxes =
                            c.optJSONArray("mailboxes")?.let { a ->
                                (0 until a.length()).map {
                                    val m = a.getJSONObject(it)
                                    InviteMailbox(
                                        MailboxKind.valueOf(m.getString("kind").uppercase()),
                                        m.getString("url"),
                                    )
                                }
                            } ?: emptyList(),
                        allowDevLoopback = c.optBoolean("allow_dev_loopback", false),
                    )
                )
            val steps = trace.getJSONArray("steps")
            val expect = trace.getJSONArray("expect")
            for (n in 0 until steps.length()) {
                val step = steps.getJSONObject(n)
                val got =
                    link.handle(
                        step.getLong("now").toULong(),
                        linkInput(step.getJSONObject("input")),
                    )
                val want =
                    (0 until expect.getJSONArray(n).length()).map {
                        expectedSummary(expect.getJSONArray(n).getJSONObject(it))
                    }
                assertEquals("${f.name} step $n", want, got.map(::actionSummary))
            }
        }
    }

    private fun hexBytes(text: String) = text.chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private fun linkInput(i: JSONObject): LinkInput {
        val attempt = i.optLong("attempt").toUInt()
        return when (val type = i.getString("type")) {
            "start" -> LinkInput.Start(i.getLong("entropy").toULong())
            "tick" -> LinkInput.Tick
            "wall_clock" -> LinkInput.WallClock(i.getLong("unix_s").toULong())
            "network_available" -> LinkInput.NetworkAvailable
            "mailbox_opened" -> LinkInput.MailboxOpened(i.getLong("id").toUInt())
            "mailbox_closed" ->
                LinkInput.MailboxClosed(i.getLong("id").toUInt(), i.getString("reason"))
            "mailbox_frame" ->
                LinkInput.MailboxFrame(i.getLong("id").toUInt(), i.getString("bytes"))
            "attempt_local_description" ->
                LinkInput.AttemptLocalDescription(
                    attempt,
                    i.getString("sdp_type"),
                    i.getString("sdp"),
                )
            "attempt_data_open" -> LinkInput.AttemptDataOpen(attempt)
            "attempt_data_message" -> LinkInput.AttemptDataMessage(attempt, i.getString("text"))
            "attempt_dtls_fingerprint" ->
                LinkInput.AttemptDtlsFingerprint(attempt, i.getString("fingerprint"))
            "attempt_pc_state" -> LinkInput.AttemptPcState(attempt, i.getString("state"))
            else -> error("trace input $type has no typed mapping")
        }
    }

    private fun snake(name: String) = name.replace(Regex("([a-z])([A-Z])"), "$1_$2").lowercase()

    private fun detail(a: JSONObject) =
        listOf("attempt", "frame", "text", "sdp")
            .mapNotNull { k -> a.opt(k)?.let { "$k=$it" } }
            .joinToString(",")

    private fun expectedSummary(a: JSONObject): String =
        if (a.getString("type") == "emit") "emit:${a.getJSONObject("event").getString("type")}"
        else "${a.getString("type")}:${detail(a)}"

    private fun actionSummary(a: LinkAction): String {
        val kind = snake(a::class.simpleName!!)
        return when (a) {
            is LinkAction.Emit -> "emit:${snake(a.event::class.simpleName!!)}"
            is LinkAction.MailboxSend -> "$kind:frame=${a.frame}"
            is LinkAction.CreateAttempt -> "$kind:attempt=${a.attempt}"
            is LinkAction.CloseAttempt -> "$kind:attempt=${a.attempt}"
            is LinkAction.CreateOffer -> "$kind:attempt=${a.attempt}"
            is LinkAction.CreateAnswer -> "$kind:attempt=${a.attempt}"
            is LinkAction.SetRemoteDescription -> "$kind:attempt=${a.attempt},sdp=${a.sdp}"
            is LinkAction.SendData -> "$kind:attempt=${a.attempt},text=${a.text}"
            else -> "$kind:"
        }
    }

    @Test
    fun wrongPskIsRejectedThroughTypedApi() {
        val config =
            defaultSessionConfig(Role.PARENT, "r", "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8")
        val session = CoreSession(config)
        session.handle(
            1u,
            Input.SignalingMessage("""{"type":"welcome","clientId":"p","roomId":"r"}"""),
        )
        val offer =
            """{"type":"signal","from":"b","payload":{"type":"sdp","aid":"0123456789abcdef0123456789abcdef","sdp":{"type":"offer","sdp":"a=fingerprint:sha-256 ${fp("AA")}\r\n"}}}"""
        session.handle(2u, Input.SignalingMessage(offer))
        session.handle(
            3u,
            Input.LocalDescription("answer", "a=fingerprint:sha-256 ${fp("BB")}\r\n"),
        )
        session.handle(4u, Input.DataChannelOpen)
        val out =
            session.handle(
                5u,
                Input.DataChannelMessage(
                    """{"type":"pair_proof","v":3,"mac":"${"f".repeat(64)}"}"""
                ),
            )
        assertTrue(
            out.any { it is Action.Emit && it.event == SessionEvent.Failed("pair_proof_mismatch") }
        )
    }

    private fun fp(byte: String) = List(32) { byte }.joinToString(":")

    private fun parseJson(s: String): Any = org.json.JSONTokener(s).nextValue()

    /** Structural equality; numbers compare by value (1.0 == 1) like JSON itself. */
    private fun assertJsonEquals(path: String, expected: Any?, actual: Any?) {
        when {
            expected is JSONObject && actual is JSONObject -> {
                val keys = expected.keys().asSequence().toSet()
                assertEquals("$path keys", keys, actual.keys().asSequence().toSet())
                for (k in keys) assertJsonEquals("$path.$k", expected.get(k), actual.get(k))
            }
            expected is JSONArray && actual is JSONArray -> {
                assertEquals("$path length", expected.length(), actual.length())
                for (i in 0 until expected.length()) assertJsonEquals(
                    "$path[$i]",
                    expected.get(i),
                    actual.get(i),
                )
            }
            expected is Number && actual is Number ->
                assertEquals(path, expected.toDouble(), actual.toDouble(), 0.0)
            else -> assertEquals(path, expected, actual)
        }
    }
}
