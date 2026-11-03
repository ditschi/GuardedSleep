package org.guardedsleep.app.ui

import androidx.compose.ui.graphics.asAndroidBitmap
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.InvertedLuminanceSource
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import org.guardedsleep.app.ui.screens.renderQr
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The invite code must stay scannable when shown the way the baby screen shows it: light modules on
 * near-black, and a dimmed screen. These tests render the real `renderQr` bitmap, dim it to 50 %
 * brightness and decode it with zxing (the same library family as the parent's scanner).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35])
class QrScanTest {
    private companion object {
        // A scanner sees many frames at different distances; one decoding frame is a scan.
        val FRAME_SIZES = listOf(320, 360, 400, 440)
    }

    // A throw-away counting-byte key, built rather than written out (the secret scan matches
    // literals).
    private val key =
        java.util.Base64.getUrlEncoder()
            .withoutPadding()
            .encodeToString(ByteArray(32) { it.toByte() })

    private fun invite(entries: List<String>) =
        """{"v":4,"k":"$key","m":[${entries.joinToString(",") { "\"$it\"" }}],"x":1900000000}"""

    /** Five entries as a full invite carries them: a relay, three further relays, a LAN address. */
    private val full =
        invite(
            listOf(
                "r:signaling.gs-operator.example.org:8443/gs/signal",
                "r:relay-one.operator-a.example.eu",
                "r:relay-two.operator-b.example.ch",
                "r:relay-three.operator-c.example.de",
                "l:192.168.178.100:8787",
            )
        )

    /** A host of the given length whose labels stay at most 50 characters (a valid DNS name). */
    private fun host(length: Int) =
        (0 until length)
            .map { if ((it + 1) % 51 == 0) '.' else "abcdefghijklmnopqrstuvwxyz"[it % 26] }
            .joinToString("")

    /** Exactly the 300 byte limit. */
    private fun atTheLimit(): String {
        val base = invite(listOf("r:.example")).length
        return invite(listOf("r:${host(300 - base)}.example"))
    }

    /**
     * A camera frame of the screen: the bitmap averaged down to [side] pixels (the sharp, pixel
     * exact bitmap trips zxing's detector at some sizes, which says nothing about the code), and
     * dimmed to [brightness].
     */
    private fun cameraFrame(pixels: IntArray, size: Int, side: Int, brightness: Float): IntArray =
        IntArray(side * side) { i ->
            val x0 = (i % side) * size / side
            val x1 = maxOf(x0 + 1, ((i % side) + 1) * size / side)
            val y0 = (i / side) * size / side
            val y1 = maxOf(y0 + 1, ((i / side) + 1) * size / side)
            var sum = 0
            for (y in y0 until y1) for (x in x0 until x1) sum += pixels[y * size + x] and 0xFF
            val grey = (sum / ((x1 - x0) * (y1 - y0)) * brightness).toInt()
            (0xFF shl 24) or (grey shl 16) or (grey shl 8) or grey
        }

    private fun scan(payload: String, bright: Boolean, brightness: Float): String {
        val bitmap = checkNotNull(renderQr(payload, bright)).asAndroidBitmap()
        check(bitmap.width == bitmap.height)
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        var failure: Exception? = null
        for (side in FRAME_SIZES) {
            val frame = cameraFrame(pixels, bitmap.width, side, brightness)
            // Light on dark: scanners read it through the inverted image.
            val source = InvertedLuminanceSource(RGBLuminanceSource(side, side, frame))
            try {
                return QRCodeReader()
                    .decode(
                        BinaryBitmap(HybridBinarizer(source)),
                        mapOf(DecodeHintType.TRY_HARDER to true),
                    )
                    .text
            } catch (e: com.google.zxing.ReaderException) {
                failure = e
            }
        }
        throw checkNotNull(failure)
    }

    @Test
    fun theFullInviteIsWithinTheBudget() {
        assertTrue(full.length in 250..300)
        assertEquals(300, atTheLimit().length)
    }

    @Test fun decodesInvertedAtHalfBrightness() = assertEquals(full, scan(full, false, 0.5f))

    @Test fun decodesTheBrightVariantAtHalfBrightness() = assertEquals(full, scan(full, true, 0.5f))

    @Test
    fun decodesAnInviteOfExactly300Bytes() {
        val text = atTheLimit()
        assertEquals(text, scan(text, false, 0.5f))
    }
}
