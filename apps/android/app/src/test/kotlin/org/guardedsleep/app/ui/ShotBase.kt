package org.guardedsleep.app.ui

import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import org.guardedsleep.app.ui.model.CauseHint
import org.guardedsleep.app.ui.model.LinkPhase
import org.guardedsleep.app.ui.model.LinkQuality
import org.guardedsleep.app.ui.model.LinkUi
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Base for Roborazzi screenshot tests: Robolectric native graphics, Pixel 6 qualifiers. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], qualifiers = RobolectricDeviceQualifiers.Pixel6)
abstract class ShotBase {}

fun link(
    phase: LinkPhase,
    level: Int = 0,
    agoMs: Long? = null,
    ack: Boolean = false,
    cause: CauseHint? = null,
    quality: LinkQuality = LinkQuality.UNKNOWN,
    failureKey: String? = null,
) =
    LinkUi(
        phase = phase,
        level = level,
        lastAliveElapsedMs = agoMs?.let { ELAPSED - it },
        // The core's outage fields for the default thresholds (20 s, 60 s, 180 s from the start).
        downSinceElapsedMs = agoMs?.let { ELAPSED - it },
        nextLevelAtElapsedMs =
            agoMs
                ?.takeIf { !ack && level < 3 }
                ?.let {
                    ELAPSED - it + longArrayOf(20_000, 60_000, 180_000)[level.coerceAtMost(2)]
                },
        acknowledgedUntilElapsedMs = if (ack) ELAPSED + 240_000 else null,
        causeHint = cause,
        failureKey = failureKey,
        quality = quality,
    )

val Connected = link(LinkPhase.CONNECTED, agoMs = 1_000)
