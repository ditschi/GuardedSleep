package org.guardedsleep.app.ui.common

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathOperation
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.sin

/**
 * Small hand-drawn symbol set (no icon dependency). Colour is never the only carrier of state, so
 * each state uses one of these next to its text (alpha-ux-flows §6.1). Glyphs are decorative; the
 * adjacent text is the accessible name.
 */
enum class Glyph {
    LINK,
    BROKEN_LINK,
    REFRESH,
    PAUSE,
    MOON,
    SUN,
    BELL,
    QUESTION,
    CHECK,
    CROSS,
    EXCLAIM,
    HEADPHONES,
    LOCK,
    MIC,
    BATTERY,
    DOTS,
    BACK,
    GEAR,
    WAVE,
    ZZ,
    STOP,
}

@Composable
fun GlyphIcon(glyph: Glyph, tint: Color, modifier: Modifier = Modifier, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) { drawGlyph(glyph, tint) }
}

private fun DrawScope.drawGlyph(g: Glyph, c: Color) {
    val u = size.minDimension / 24f
    val sw = 2f * u
    val st = Stroke(width = sw, cap = StrokeCap.Round, join = StrokeJoin.Round)
    fun p(x: Float, y: Float) = Offset(x * u, y * u)
    when (g) {
        Glyph.LINK -> {
            drawRoundRect(c, p(2f, 8f), Size(12f * u, 8f * u), CornerRadius(4f * u), style = st)
            drawRoundRect(c, p(10f, 8f), Size(12f * u, 8f * u), CornerRadius(4f * u), style = st)
        }
        Glyph.BROKEN_LINK -> {
            drawRoundRect(c, p(1.5f, 9f), Size(9f * u, 6f * u), CornerRadius(3f * u), style = st)
            drawRoundRect(c, p(13.5f, 9f), Size(9f * u, 6f * u), CornerRadius(3f * u), style = st)
            drawLine(c, p(12f, 4f), p(12f, 7f), sw, StrokeCap.Round)
            drawLine(c, p(12f, 17f), p(12f, 20f), sw, StrokeCap.Round)
        }
        Glyph.REFRESH -> {
            drawArc(c, 40f, 290f, false, p(4f, 4f), Size(16f * u, 16f * u), style = st)
            val tip = p(19.2f, 6.8f)
            drawLine(c, tip, p(19.8f, 2.5f), sw, StrokeCap.Round)
            drawLine(c, tip, p(15.5f, 7.3f), sw, StrokeCap.Round)
        }
        Glyph.PAUSE -> {
            drawRoundRect(c, p(6f, 5f), Size(4f * u, 14f * u), CornerRadius(1f * u))
            drawRoundRect(c, p(14f, 5f), Size(4f * u, 14f * u), CornerRadius(1f * u))
        }
        Glyph.MOON -> {
            val a =
                Path().apply {
                    addOval(androidx.compose.ui.geometry.Rect(p(4f, 3f), Size(16f * u, 18f * u)))
                }
            val b =
                Path().apply {
                    addOval(androidx.compose.ui.geometry.Rect(p(10f, 1f), Size(14f * u, 16f * u)))
                }
            drawPath(Path.combine(PathOperation.Difference, a, b), c)
        }
        Glyph.SUN -> {
            drawCircle(c, 4f * u, p(12f, 12f), style = st)
            for (i in 0 until 8) {
                val a = Math.toRadians(i * 45.0)
                drawLine(
                    c,
                    p(12f + 7f * cos(a).toFloat(), 12f + 7f * sin(a).toFloat()),
                    p(12f + 10f * cos(a).toFloat(), 12f + 10f * sin(a).toFloat()),
                    sw,
                    StrokeCap.Round,
                )
            }
        }
        Glyph.BELL -> {
            val path =
                Path().apply {
                    moveTo(6f * u, 17f * u)
                    lineTo(6f * u, 11f * u)
                    cubicTo(6f * u, 6f * u, 18f * u, 6f * u, 18f * u, 11f * u)
                    lineTo(18f * u, 17f * u)
                    close()
                }
            drawPath(path, c, style = st)
            drawLine(c, p(4f, 17f), p(20f, 17f), sw, StrokeCap.Round)
            drawArc(c, 0f, 180f, false, p(9.5f, 17.5f), Size(5f * u, 4f * u), style = st)
        }
        Glyph.QUESTION -> {
            drawArc(c, 180f, 220f, false, p(7.5f, 3.5f), Size(9f * u, 9f * u), style = st)
            drawLine(c, p(12f, 12f), p(12f, 15f), sw, StrokeCap.Round)
            drawCircle(c, 1.2f * u, p(12f, 19.5f))
        }
        Glyph.CHECK -> {
            val path =
                Path().apply {
                    moveTo(4f * u, 12.5f * u)
                    lineTo(10f * u, 18f * u)
                    lineTo(20f * u, 6f * u)
                }
            drawPath(
                path,
                c,
                style = Stroke(width = 2.6f * u, cap = StrokeCap.Round, join = StrokeJoin.Round),
            )
        }
        Glyph.CROSS -> {
            drawLine(c, p(6f, 6f), p(18f, 18f), 2.6f * u, StrokeCap.Round)
            drawLine(c, p(18f, 6f), p(6f, 18f), 2.6f * u, StrokeCap.Round)
        }
        Glyph.EXCLAIM -> {
            drawLine(c, p(12f, 4f), p(12f, 14f), 3f * u, StrokeCap.Round)
            drawCircle(c, 1.8f * u, p(12f, 19f))
        }
        Glyph.HEADPHONES -> {
            drawArc(c, 180f, 180f, false, p(4f, 4f), Size(16f * u, 16f * u), style = st)
            drawRoundRect(c, p(3f, 12f), Size(4f * u, 7f * u), CornerRadius(2f * u))
            drawRoundRect(c, p(17f, 12f), Size(4f * u, 7f * u), CornerRadius(2f * u))
        }
        Glyph.LOCK -> {
            drawRoundRect(c, p(5f, 11f), Size(14f * u, 10f * u), CornerRadius(2f * u))
            drawArc(c, 180f, 180f, false, p(8f, 3f), Size(8f * u, 10f * u), style = st)
        }
        Glyph.MIC -> {
            drawRoundRect(c, p(9f, 2f), Size(6f * u, 12f * u), CornerRadius(3f * u))
            drawArc(c, 0f, 180f, false, p(5f, 6f), Size(14f * u, 12f * u), style = st)
            drawLine(c, p(12f, 18f), p(12f, 22f), sw, StrokeCap.Round)
        }
        Glyph.BATTERY -> {
            drawRoundRect(c, p(2f, 7f), Size(18f * u, 10f * u), CornerRadius(2f * u), style = st)
            drawRoundRect(c, p(20.5f, 10.5f), Size(2f * u, 3f * u), CornerRadius(0.5f * u))
            drawRoundRect(c, p(4.5f, 9.5f), Size(8f * u, 5f * u), CornerRadius(1f * u))
        }
        Glyph.DOTS -> {
            for (y in listOf(5f, 12f, 19f)) drawCircle(c, 2f * u, p(12f, y))
        }
        Glyph.BACK -> {
            val path =
                Path().apply {
                    moveTo(14f * u, 5f * u)
                    lineTo(7f * u, 12f * u)
                    lineTo(14f * u, 19f * u)
                }
            drawPath(
                path,
                c,
                style = Stroke(width = 2.6f * u, cap = StrokeCap.Round, join = StrokeJoin.Round),
            )
        }
        Glyph.GEAR -> {
            drawCircle(c, 4f * u, p(12f, 12f), style = st)
            for (i in 0 until 8) {
                val a = Math.toRadians(i * 45.0)
                drawLine(
                    c,
                    p(12f + 7f * cos(a).toFloat(), 12f + 7f * sin(a).toFloat()),
                    p(12f + 10f * cos(a).toFloat(), 12f + 10f * sin(a).toFloat()),
                    3f * u,
                    StrokeCap.Round,
                )
            }
        }
        Glyph.WAVE -> {
            val hs = listOf(6f, 12f, 18f, 12f, 6f)
            hs.forEachIndexed { i, h ->
                val x = 3.5f + i * 4.25f
                drawLine(c, p(x, 12f - h / 2), p(x, 12f + h / 2), 2.6f * u, StrokeCap.Round)
            }
        }
        Glyph.STOP -> drawRoundRect(c, p(5f, 5f), Size(14f * u, 14f * u), CornerRadius(2.5f * u))
        Glyph.ZZ -> {
            val path =
                Path().apply {
                    moveTo(4f * u, 5f * u)
                    lineTo(12f * u, 5f * u)
                    lineTo(4f * u, 13f * u)
                    lineTo(12f * u, 13f * u)
                    moveTo(13f * u, 14f * u)
                    lineTo(20f * u, 14f * u)
                    lineTo(13f * u, 21f * u)
                    lineTo(20f * u, 21f * u)
                }
            drawPath(
                path,
                c,
                style = Stroke(width = 2f * u, cap = StrokeCap.Round, join = StrokeJoin.Round),
            )
        }
    }
}
