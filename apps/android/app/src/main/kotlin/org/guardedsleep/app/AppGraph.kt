package org.guardedsleep.app

import android.content.Context
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import org.guardedsleep.app.alerts.AlertController
import org.guardedsleep.app.link.LinkController
import org.guardedsleep.app.link.SystemClockSource
import org.guardedsleep.app.media.LibWebRtcMediaEngine
import org.guardedsleep.app.pairing.KeystorePairingCipher
import org.guardedsleep.app.pairing.PairingStore
import org.guardedsleep.app.readiness.AndroidReadinessEnv
import org.guardedsleep.app.readiness.ReadinessChecker
import org.guardedsleep.app.service.AndroidNetworkWatcher
import org.guardedsleep.app.service.ServiceLauncher
import org.guardedsleep.app.settings.SettingsRepository
import org.guardedsleep.app.signaling.OkHttpSignalingFactory
import org.guardedsleep.app.silence.SilenceController

/** Manual dependency graph; one per process, created by [GuardedSleepApp]. */
@OptIn(ExperimentalCoroutinesApi::class)
class AppGraph(context: Context) {
    private val app = context.applicationContext
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO.limitedParallelism(1))

    val settings =
        SettingsRepository(
            PreferenceDataStoreFactory.create(scope = ioScope) {
                File(app.filesDir, "datastore/gs_settings.preferences_pb")
            },
            ioScope,
        )
    val pairingStore = PairingStore(File(app.filesDir, "pairing.bin"), KeystorePairingCipher())
    val silence = SilenceController(app, settings)
    val alerts = AlertController(app)
    val readiness = ReadinessChecker(AndroidReadinessEnv(app), settings, SystemClockSource)
    val controller =
        LinkController(
            settingsRepo = settings,
            pairingStore = pairingStore,
            signaling = OkHttpSignalingFactory(),
            mediaFactory = { role, ice -> LibWebRtcMediaEngine(app, role, ice) },
            silence = silence,
            alerts = alerts,
            readiness = readiness,
            host = ServiceLauncher(app),
            network = AndroidNetworkWatcher(app),
        )
}

fun Context.graph(): AppGraph = (applicationContext as GuardedSleepApp).graph
