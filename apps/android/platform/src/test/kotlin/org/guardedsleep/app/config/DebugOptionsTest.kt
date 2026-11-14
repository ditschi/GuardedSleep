package org.guardedsleep.app.config

import org.guardedsleep.app.platform.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Test

class DebugOptionsTest {
    // The UI journey of the emulator gate (AC-12) switches relay-only media on through a debug-only
    // launch extra; a release build must never honour it, whatever was set.
    @Test
    fun relayOnlyIsEffectiveOnlyInDebugBuilds() {
        DebugOptions.setRelayOnly(true)
        try {
            assertEquals(BuildConfig.DEBUG, DebugOptions.relayOnly)
        } finally {
            DebugOptions.setRelayOnly(false)
        }
        assertEquals(false, DebugOptions.relayOnly)
    }
}
