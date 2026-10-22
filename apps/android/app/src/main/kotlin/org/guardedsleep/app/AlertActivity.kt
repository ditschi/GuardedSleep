package org.guardedsleep.app

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import org.guardedsleep.app.ui.LinkAlarmScreen
import org.guardedsleep.app.ui.NoiseAlertScreen
import org.guardedsleep.app.wiring.AlertIntents

/** Full-screen alert over the lock screen, launched by the alarm notifications. */
@OptIn(ExperimentalComposeUiApi::class)
class AlertActivity : ComponentActivity() {
    private var kind by mutableStateOf(AlertIntents.KIND_LINK)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setShowWhenLocked(true)
        setTurnScreenOn(true)
        kind = intent.getStringExtra(AlertIntents.EXTRA_KIND) ?: AlertIntents.KIND_LINK
        val controller = graph().controller
        setContent {
            Box(Modifier.fillMaxSize().semantics { testTagsAsResourceId = true }) {
                if (kind == AlertIntents.KIND_NOISE)
                    NoiseAlertScreen(controller, onClose = ::finish)
                else LinkAlarmScreen(controller, onClose = ::finish)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        kind = intent.getStringExtra(AlertIntents.EXTRA_KIND) ?: AlertIntents.KIND_LINK
    }
}
