package org.guardedsleep.app

import android.content.Context
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.guardedsleep.app.ui.GuardedSleepRoot
import org.guardedsleep.app.ui.model.Role

@OptIn(ExperimentalComposeUiApi::class)
class MainActivity : ComponentActivity() {
    override fun attachBaseContext(base: Context) {
        // Android 12 and older: start in the stored language (the settings load synchronously).
        val app = base.applicationContext as? GuardedSleepApp
        super.attachBaseContext(
            AppLocale.wrap(base, app?.graph?.settings?.current?.settings?.language)
        )
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val controller = graph().controller
        // The language choice applies at once (per-app locale API, see AppLocale).
        lifecycleScope.launch {
            controller.settings
                .map { it.language }
                .distinctUntilChanged()
                .collect { AppLocale.apply(this@MainActivity, it) }
        }
        // The baby phone never vibrates or clicks, not even for a touch (ADR-018 §3.1).
        lifecycleScope.launch {
            controller.app
                .map { it.role == Role.BABY }
                .collect { baby ->
                    window.decorView.isHapticFeedbackEnabled = !baby
                    window.decorView.isSoundEffectsEnabled = !baby
                }
        }
        // The pairing code is a secret: no screenshots, recents thumbnail or screen recording while
        // it is shown.
        lifecycleScope.launch {
            controller.pairing
                .map { it?.codePayload != null }
                .collect { shown ->
                    if (shown) window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    else window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                }
        }
        setContent {
            Box(Modifier.fillMaxSize().semantics { testTagsAsResourceId = true }) {
                GuardedSleepRoot(controller)
            }
        }
    }

    override fun onStart() {
        super.onStart()
        graph().controller.onForeground()
    }

    override fun onResume() {
        super.onResume()
        graph().controller.refreshReadiness()
    }
}
