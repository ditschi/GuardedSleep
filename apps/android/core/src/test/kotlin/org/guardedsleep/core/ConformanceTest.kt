package org.guardedsleep.core

import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.guardedsleep_core.Action
import uniffi.guardedsleep_core.CoreSession
import uniffi.guardedsleep_core.Input
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
        val files = File(System.getProperty("conformance.traces")!!)
            .listFiles { f -> f.extension == "json" }!!.sortedBy { it.name }
        assertTrue(files.isNotEmpty())
        for (f in files) {
            val trace = JSONObject(f.readText())
            val got = parseJson(conformanceEval(trace.toString()))
            assertJsonEquals(f.name, trace.get("expect"), got)
        }
    }

    @Test
    fun wrongPskIsRejectedThroughTypedApi() {
        val config = defaultSessionConfig(Role.PARENT, "r", "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8")
        val session = CoreSession(config)
        session.handle(1u, Input.SignalingMessage("""{"type":"welcome","clientId":"p","roomId":"r"}"""))
        val offer = """{"type":"signal","from":"b","payload":{"type":"sdp","aid":"0123456789abcdef0123456789abcdef","sdp":{"type":"offer","sdp":"a=fingerprint:sha-256 ${fp("AA")}\r\n"}}}"""
        session.handle(2u, Input.SignalingMessage(offer))
        session.handle(3u, Input.LocalDescription("answer", "a=fingerprint:sha-256 ${fp("BB")}\r\n"))
        session.handle(4u, Input.DataChannelOpen)
        val out = session.handle(5u, Input.DataChannelMessage("""{"type":"pair_proof","v":3,"mac":"${"f".repeat(64)}"}"""))
        assertTrue(out.any { it is Action.Emit && it.event == SessionEvent.Failed("pair_proof_mismatch") })
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
                for (i in 0 until expected.length()) assertJsonEquals("$path[$i]", expected.get(i), actual.get(i))
            }
            expected is Number && actual is Number ->
                assertEquals(path, expected.toDouble(), actual.toDouble(), 0.0)
            else -> assertEquals(path, expected, actual)
        }
    }
}
