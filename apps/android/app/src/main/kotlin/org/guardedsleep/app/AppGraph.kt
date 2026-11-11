package org.guardedsleep.app

import android.content.Context
import android.os.Build
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.guardedsleep.app.alerts.AlertChannels
import org.guardedsleep.app.alerts.AlertController
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.link.SystemClockSource
import org.guardedsleep.app.media.libwebrtc.LibWebRtcMediaEngine
import org.guardedsleep.app.pairing.KeystorePairingCipher
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.pairing.RolePairings
import org.guardedsleep.app.pairing.TurnStore
import org.guardedsleep.app.readiness.AndroidReadinessEnv
import org.guardedsleep.app.readiness.ReadinessChecker
import org.guardedsleep.app.service.AndroidNetworkWatcher
import org.guardedsleep.app.service.ServiceLauncher
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.signaling.OkHttpSignalingFactory
import org.guardedsleep.app.silence.SilenceController
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.wiring.EntryPoints
import org.guardedsleep.app.wiring.PlatformServices

/** Manual dependency graph; one per process, created by [GuardedSleepApp]. */
@OptIn(ExperimentalCoroutinesApi::class)
class AppGraph(context: Context) : PlatformServices {
    private val app = context.applicationContext
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO.limitedParallelism(1))

    // Sealed apart from the pairing: forgetting one must not delete the other.
    val turnStore =
        TurnStore(
            File(app.filesDir, TurnStore.FILE_NAME),
            KeystorePairingCipher(TurnStore.KEY_ALIAS),
        )
    override val settings =
        SettingsRepository(
            PreferenceDataStoreFactory.create(scope = ioScope) {
                File(app.filesDir, "datastore/gs_settings.preferences_pb")
            },
            ioScope,
            turnStore,
        )
    // One sealed file and one Keystore key per role, so forgetting one never deletes the other.
    // The legacy single file (pairing.bin) moves into the slot of the persisted role on start.
    val pairings =
        RolePairings(
            Role.entries.associateWith {
                val name = it.name.lowercase()
                PairingStore(
                    File(app.filesDir, "pairing-$name.bin"),
                    KeystorePairingCipher("${KeystorePairingCipher.KEY_ALIAS}_$name"),
                )
            },
            legacy = PairingStore(File(app.filesDir, "pairing.bin"), KeystorePairingCipher()),
        )
    override val entryPoints: EntryPoints = AppEntryPoints
    val silence = SilenceController(app, settings, entryPoints)
    val alerts = AlertController(app, entryPoints) { settings.current.settings.language }
    val readiness = ReadinessChecker(AndroidReadinessEnv(app), settings, SystemClockSource)

    init {
        // Notification channel names follow the app language as soon as it changes, not only with
        // the next alert.
        ioScope.launch {
            settings.flow
                .map { it.settings.language }
                .distinctUntilChanged()
                .collect { AlertChannels.ensure(app, it) }
        }
    }

    override val controller =
        LinkController(
            settingsRepo = settings,
            pairings = pairings,
            turnStore = turnStore,
            signaling = OkHttpSignalingFactory(),
            mediaFactory = { role, ice -> LibWebRtcMediaEngine(app, role, ice) },
            silence = silence,
            alerts = alerts,
            readiness = readiness,
            host = ServiceLauncher(app),
            network = AndroidNetworkWatcher(app),
            // The model name the user knows the phone by, shown on the baby device; no identifier.
            deviceName = { Build.MODEL.orEmpty() },
        )
}

/** The `app` components behind the platform notifications. */
private object AppEntryPoints : EntryPoints {
    override val mainActivity = MainActivity::class.java
    override val alertActivity = AlertActivity::class.java
    override val alertActionReceiver = AlertActionReceiver::class.java
}

fun Context.graph(): AppGraph = (applicationContext as GuardedSleepApp).graph
