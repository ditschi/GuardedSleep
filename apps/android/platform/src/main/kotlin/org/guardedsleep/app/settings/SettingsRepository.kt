package org.guardedsleep.app.settings

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.guardedsleep.app.config.AppDefaults
import org.guardedsleep.app.pairing.TurnCredentials
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.ui.model.AlertOutput
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.model.SettingsState
import org.guardedsleep.app.ui.model.Theme

/**
 * Do Not Disturb state this app switched on, persisted so a crash can be undone at the next start.
 */
data class SilenceMarker(
    val active: Boolean = false,
    val ruleId: String? = null,
    val previousFilter: Int? = null,
)

/**
 * Everything that survives a process restart and is not secret (pairing material is in
 * PairingStore).
 */
data class PersistedState(
    val settings: SettingsState,
    val role: Role? = null,
    val onboardingDone: Boolean = false,
    /** "Monitoring is on": the parent service resumes it, the baby offers "Tap to resume". */
    val monitoringActive: Boolean = false,
    val peerName: String? = null,
    /** Snooze end as wall clock; converted to the monotonic clock when a run starts. */
    val snoozeUntilWallMs: Long? = null,
    val lastTestWallMs: Long? = null,
    val silence: SilenceMarker = SilenceMarker(),
)

/**
 * DataStore-backed preferences with an in-memory mirror: reads are synchronous, writes are applied
 * immediately and persisted in the background.
 */
class SettingsRepository(
    private val store: DataStore<Preferences>,
    private val scope: CoroutineScope,
    /** Receives a legacy plain `turn_url` once; null in tests that do not care. */
    private val turnStore: TurnStore? = null,
) {
    private val state =
        MutableStateFlow(
            runBlocking {
                migrateLegacyTurn()
                read(store.data.first())
            }
        )

    val flow: StateFlow<PersistedState> = state

    val current: PersistedState
        get() = state.value

    fun update(transform: (PersistedState) -> PersistedState) {
        val next = state.updateAndGet(transform)
        scope.launch { store.edit { write(it, next) } }
    }

    private inline fun <T> MutableStateFlow<T>.updateAndGet(f: (T) -> T): T {
        var result: T
        while (true) {
            val prev = value
            result = f(prev)
            if (compareAndSet(prev, result)) return result
        }
    }

    /** Moves the pre-TURN-credentials `turn_url` preference into the sealed store, once. */
    private suspend fun migrateLegacyTurn() {
        val legacy = store.data.first()[TURN] ?: return
        val turnStore = turnStore ?: return
        val kept = runCatching {
            if (turnStore.load() == null && legacy.isNotBlank()) {
                turnStore.save(TurnCredentials(legacy, "", ""))
            }
        }
            .isSuccess
        // A failed move keeps the key, so the next start retries.
        if (kept) store.edit { it.remove(TURN) }
    }

    private fun read(p: Preferences): PersistedState {
        val defaults = SettingsState(signalingUrls = AppDefaults.signalingUrls)
        val settings =
            SettingsState(
                theme =
                    p[THEME]?.let { runCatching { Theme.valueOf(it) }.getOrNull() }
                        ?: defaults.theme,
                language = p[LANGUAGE],
                threshold = p[THRESHOLD] ?: defaults.threshold,
                holdSeconds = p[HOLD] ?: defaults.holdSeconds,
                alertOutput =
                    p[OUTPUT]?.let { runCatching { AlertOutput.valueOf(it) }.getOrNull() }
                        ?: defaults.alertOutput,
                alertStyle =
                    p[STYLE]?.let { runCatching { AlertStyle.valueOf(it) }.getOrNull() }
                        ?: defaults.alertStyle,
                alertGainPct = p[GAIN] ?: defaults.alertGainPct,
                silenceWhileMonitoring = p[SILENCE] ?: defaults.silenceWhileMonitoring,
                advancedEnabled = p[ADVANCED] ?: defaults.advancedEnabled,
                signalingUrls =
                    p[URLS]?.split('\n')?.filter { it.isNotBlank() } ?: defaults.signalingUrls,
                onlyLocal = p[ONLY_LOCAL] ?: defaults.onlyLocal,
                heartbeatSeconds = p[HEARTBEAT] ?: defaults.heartbeatSeconds,
            )
        return PersistedState(
            settings = settings,
            role = p[ROLE]?.let { runCatching { Role.valueOf(it) }.getOrNull() },
            onboardingDone = p[ONBOARDING] ?: false,
            monitoringActive = p[MONITORING] ?: false,
            peerName = p[PEER_NAME],
            snoozeUntilWallMs = p[SNOOZE],
            lastTestWallMs = p[LAST_TEST],
            silence = SilenceMarker(p[SILENCE_ACTIVE] ?: false, p[SILENCE_RULE], p[SILENCE_PREV]),
        )
    }

    private fun write(
        p: androidx.datastore.preferences.core.MutablePreferences,
        s: PersistedState,
    ) {
        fun <T> set(key: Preferences.Key<T>, v: T?) {
            if (v == null) p.remove(key) else p[key] = v
        }
        val c = s.settings
        set(THEME, c.theme.name)
        set(LANGUAGE, c.language)
        set(THRESHOLD, c.threshold)
        set(HOLD, c.holdSeconds)
        set(OUTPUT, c.alertOutput.name)
        set(STYLE, c.alertStyle.name)
        set(GAIN, c.alertGainPct)
        set(SILENCE, c.silenceWhileMonitoring)
        set(ADVANCED, c.advancedEnabled)
        set(URLS, c.signalingUrls.joinToString("\n"))
        set(ONLY_LOCAL, c.onlyLocal)
        set(HEARTBEAT, c.heartbeatSeconds)
        set(ROLE, s.role?.name)
        set(ONBOARDING, s.onboardingDone)
        set(MONITORING, s.monitoringActive)
        set(PEER_NAME, s.peerName)
        set(SNOOZE, s.snoozeUntilWallMs)
        set(LAST_TEST, s.lastTestWallMs)
        set(SILENCE_ACTIVE, s.silence.active)
        set(SILENCE_RULE, s.silence.ruleId)
        set(SILENCE_PREV, s.silence.previousFilter)
    }

    private companion object {
        val THEME = stringPreferencesKey("theme")
        val LANGUAGE = stringPreferencesKey("language")
        val THRESHOLD = floatPreferencesKey("threshold")
        val HOLD = intPreferencesKey("hold_seconds")
        val OUTPUT = stringPreferencesKey("alert_output")
        val STYLE = stringPreferencesKey("alert_style")
        val GAIN = intPreferencesKey("alert_gain_pct")
        val SILENCE = booleanPreferencesKey("silence_while_monitoring")
        val ADVANCED = booleanPreferencesKey("advanced")
        val URLS = stringPreferencesKey("signaling_urls")
        val ONLY_LOCAL = booleanPreferencesKey("only_local")
        val TURN = stringPreferencesKey("turn_url")
        val HEARTBEAT = intPreferencesKey("heartbeat_seconds")
        val ROLE = stringPreferencesKey("role")
        val ONBOARDING = booleanPreferencesKey("onboarding_done")
        val MONITORING = booleanPreferencesKey("monitoring_active")
        val PEER_NAME = stringPreferencesKey("peer_name")
        val SNOOZE = longPreferencesKey("snooze_until_wall")
        val LAST_TEST = longPreferencesKey("last_test_wall")
        val SILENCE_ACTIVE = booleanPreferencesKey("silence_active")
        val SILENCE_RULE = stringPreferencesKey("silence_rule_id")
        val SILENCE_PREV = intPreferencesKey("silence_prev_filter")
    }
}
