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
import org.guardedsleep.app.ui.model.DiscreetEscalation
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
/** The per-role values of a role that is not the session role (see [PersistedState.parked]). */
data class RoleLocal(
    val peerName: String? = null,
    val snoozeUntilWallMs: Long? = null,
    val lastTestWallMs: Long? = null,
)

data class PersistedState(
    val settings: SettingsState,
    /** The session role: what the app runs as now (MonitorService resumes it while monitoring). */
    val role: Role? = null,
    /** "Start mode": the role a cold start opens in; null asks every time. */
    val defaultRole: Role? = null,
    val onboardingDone: Boolean = false,
    /** "Monitoring is on": the parent service resumes it, the baby offers "Tap to resume". */
    val monitoringActive: Boolean = false,
    // peerName, snoozeUntilWallMs and lastTestWallMs belong to the session role. The repository
    // moves them to [parked] and back when `role` changes, and persists each under its role.
    val peerName: String? = null,
    /** Snooze end as wall clock; converted to the monotonic clock when a run starts. */
    val snoozeUntilWallMs: Long? = null,
    val lastTestWallMs: Long? = null,
    /** The per-role values of the roles other than [role] (set by the repository only). */
    val parked: Map<Role, RoleLocal> = emptyMap(),
    val silence: SilenceMarker = SilenceMarker(),
) {
    internal fun local() = RoleLocal(peerName, snoozeUntilWallMs, lastTestWallMs)
}

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
        val next = state.updateAndGet { switchRoleLocals(it, transform(it)) }
        scope.launch { store.edit { write(it, next) } }
    }

    /**
     * A role change swaps the per-role values: the old role's go to `parked`, the new role's come
     * back. Callers copy `role` without touching the three fields.
     */
    private fun switchRoleLocals(prev: PersistedState, next: PersistedState): PersistedState {
        if (prev.role == next.role) return next
        val parked = next.parked.toMutableMap()
        prev.role?.let { parked[it] = prev.local() }
        val incoming = next.role?.let { parked.remove(it) } ?: RoleLocal()
        return next.copy(
            peerName = incoming.peerName,
            snoozeUntilWallMs = incoming.snoozeUntilWallMs,
            lastTestWallMs = incoming.lastTestWallMs,
            parked = parked,
        )
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

    /** A stored enum name, or null when absent or from an unknown (older or newer) version. */
    private inline fun <reified E : Enum<E>> enumOrNull(name: String?): E? = name?.let { n ->
        runCatching { enumValueOf<E>(n) }.getOrNull()
    }

    private fun read(p: Preferences): PersistedState {
        val defaults = SettingsState(signalingUrls = AppDefaults.signalingUrls)
        val settings =
            SettingsState(
                theme = enumOrNull<Theme>(p[THEME]) ?: defaults.theme,
                language = p[LANGUAGE],
                threshold = p[THRESHOLD] ?: defaults.threshold,
                holdSeconds = p[HOLD] ?: defaults.holdSeconds,
                alertOutput = enumOrNull<AlertOutput>(p[OUTPUT]) ?: defaults.alertOutput,
                alertStyle = enumOrNull<AlertStyle>(p[STYLE]) ?: defaults.alertStyle,
                discreetEscalation =
                    enumOrNull<DiscreetEscalation>(p[ESCALATION]) ?: defaults.discreetEscalation,
                alertGainPct = p[GAIN] ?: defaults.alertGainPct,
                silenceWhileMonitoring = p[SILENCE] ?: defaults.silenceWhileMonitoring,
                advancedEnabled = p[ADVANCED] ?: defaults.advancedEnabled,
                signalingUrls =
                    p[URLS]?.split('\n')?.filter { it.isNotBlank() } ?: defaults.signalingUrls,
                onlyLocal = p[ONLY_LOCAL] ?: defaults.onlyLocal,
                heartbeatSeconds = p[HEARTBEAT] ?: defaults.heartbeatSeconds,
            )
        val role = enumOrNull<Role>(p[ROLE])
        // Update from a single-role install: the stored role becomes the start mode, and its
        // unsuffixed per-role values become that role's. The next write stores the new layout.
        val defaultRole =
            when (val d = p[DEFAULT_ROLE]) {
                null -> role
                ASK -> null
                else -> enumOrNull<Role>(d)
            }
        fun local(r: Role) =
            RoleLocal(
                peerName = p[peerKey(r)] ?: PEER_NAME.takeIf { r == role }?.let { p[it] },
                snoozeUntilWallMs = p[snoozeKey(r)] ?: SNOOZE.takeIf { r == role }?.let { p[it] },
                lastTestWallMs = p[lastTestKey(r)] ?: LAST_TEST.takeIf { r == role }?.let { p[it] },
            )
        val active = role?.let(::local) ?: RoleLocal()
        return PersistedState(
            settings = settings,
            role = role,
            defaultRole = defaultRole,
            onboardingDone = p[ONBOARDING] ?: false,
            monitoringActive = p[MONITORING] ?: false,
            peerName = active.peerName,
            snoozeUntilWallMs = active.snoozeUntilWallMs,
            lastTestWallMs = active.lastTestWallMs,
            parked = Role.entries.filter { it != role }.associateWith(::local),
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
        set(ESCALATION, c.discreetEscalation.name)
        set(GAIN, c.alertGainPct)
        set(SILENCE, c.silenceWhileMonitoring)
        set(ADVANCED, c.advancedEnabled)
        set(URLS, c.signalingUrls.joinToString("\n"))
        set(ONLY_LOCAL, c.onlyLocal)
        set(HEARTBEAT, c.heartbeatSeconds)
        set(ROLE, s.role?.name)
        set(ONBOARDING, s.onboardingDone)
        set(MONITORING, s.monitoringActive)
        set(DEFAULT_ROLE, s.defaultRole?.name ?: ASK)
        // The unsuffixed keys are the pre-roles layout; they are dropped once the new one is
        // stored.
        p.remove(PEER_NAME)
        p.remove(SNOOZE)
        p.remove(LAST_TEST)
        val locals = s.parked + (s.role?.let { mapOf(it to s.local()) } ?: emptyMap())
        for (r in Role.entries) {
            val l = locals[r] ?: RoleLocal()
            set(peerKey(r), l.peerName)
            set(snoozeKey(r), l.snoozeUntilWallMs)
            set(lastTestKey(r), l.lastTestWallMs)
        }
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
        val ESCALATION = stringPreferencesKey("discreet_escalation")
        val GAIN = intPreferencesKey("alert_gain_pct")
        val SILENCE = booleanPreferencesKey("silence_while_monitoring")
        val ADVANCED = booleanPreferencesKey("advanced")
        val URLS = stringPreferencesKey("signaling_urls")
        val ONLY_LOCAL = booleanPreferencesKey("only_local")
        val TURN = stringPreferencesKey("turn_url")
        val HEARTBEAT = intPreferencesKey("heartbeat_seconds")
        val ROLE = stringPreferencesKey("role")
        val DEFAULT_ROLE = stringPreferencesKey("default_role")
        // Stored for "Ask every time", so an absent key still means "update from a single role".
        const val ASK = "ask"
        val ONBOARDING = booleanPreferencesKey("onboarding_done")
        val MONITORING = booleanPreferencesKey("monitoring_active")
        val PEER_NAME = stringPreferencesKey("peer_name")
        val SNOOZE = longPreferencesKey("snooze_until_wall")
        val LAST_TEST = longPreferencesKey("last_test_wall")

        fun peerKey(r: Role) = stringPreferencesKey("peer_name_${r.name.lowercase()}")

        fun snoozeKey(r: Role) = longPreferencesKey("snooze_until_wall_${r.name.lowercase()}")

        fun lastTestKey(r: Role) = longPreferencesKey("last_test_wall_${r.name.lowercase()}")

        val SILENCE_ACTIVE = booleanPreferencesKey("silence_active")
        val SILENCE_RULE = stringPreferencesKey("silence_rule_id")
        val SILENCE_PREV = intPreferencesKey("silence_prev_filter")
    }
}
