package org.guardedsleep.app.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role as SemRole
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphButton
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.connectionLabel
import org.guardedsleep.app.ui.common.connectionListLabel
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.theme.LocalAppColors

/** S01: first run only. Two large cards (120 dp each). */
@Composable
fun RoleScreen(controller: AppController) {
    // Off for every start; "Use as default" sets the start mode together with the choice.
    var makeDefault by remember { mutableStateOf(false) }
    ScreenRoot("role") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Title(stringResource(R.string.native_ui_role_title), "role_title")
            RoleCard(
                "role_baby",
                Glyph.MIC,
                R.string.native_ui_role_baby_title,
                R.string.native_ui_role_baby_hint,
            ) {
                controller.chooseRole(Role.BABY, makeDefault)
            }
            RoleCard(
                "role_parent",
                Glyph.BELL,
                R.string.native_ui_role_parent_title,
                R.string.native_ui_role_parent_hint,
            ) {
                controller.chooseRole(Role.PARENT, makeDefault)
            }
            SwitchRow(
                stringResource(R.string.native_ui_role_default_toggle),
                makeDefault,
                "role_default_toggle",
            ) {
                makeDefault = it
            }
            Body(stringResource(R.string.native_ui_role_order_hint), tag = "role_order_hint")
            PrivacyLine("role_privacy_note")
        }
    }
}

/** The privacy promise: encrypted between the devices, no account, no cloud recording. */
@Composable
internal fun PrivacyLine(tag: String) {
    Row(
        Modifier.fillMaxWidth().testTag(tag),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.Top,
    ) {
        GlyphIcon(Glyph.SHIELD, LocalAppColors.current.secondaryText, size = 20.dp)
        Body(stringResource(R.string.native_ui_home_privacy))
    }
}

@Composable
private fun RoleCard(tag: String, glyph: Glyph, title: Int, hint: Int, onClick: () -> Unit) {
    AppCard(Modifier.testTag(tag).heightIn(min = 120.dp), onClick = onClick) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 88.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            GlyphIcon(glyph, MaterialTheme.colorScheme.primary, size = 40.dp)
            Column(Modifier.weight(1f)) {
                Text(
                    stringResource(title),
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Body(stringResource(hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** "Switch mode": back to the role screen; the caller shows it only while not monitoring. */
@Composable
internal fun SwitchModeButton(controller: AppController, modifier: Modifier = Modifier) {
    SecondaryButton(
        stringResource(R.string.native_ui_home_switch_mode),
        { controller.switchMode() },
        "switch_mode",
        modifier,
    )
}

/** S06: saved pairing → one-tap start; otherwise the empty state. */
@Composable
fun HomeScreen(
    controller: AppController,
    onSettings: () -> Unit,
    /** True once the parent's empty home opened the scan screen for this entry. */
    autoScanned: Boolean = true,
    onAutoScanned: () -> Unit = {},
    /** "Choose another connection" just stopped monitoring: open with the list expanded. */
    listRequested: Boolean = false,
    onListShown: () -> Unit = {},
) {
    val app = controller.app.collectAsState().value
    val isParent = app.role == Role.PARENT
    // `device-setup`, "Parent pairing entry": with nothing saved the parent goes straight to the
    // scan screen, once. Cancelling lands here again; the flag keeps it from reopening.
    LaunchedEffect(isParent, app.connections.isEmpty(), autoScanned) {
        if (isParent && app.connections.isEmpty() && app.canAddPairing && !autoScanned) {
            onAutoScanned()
            controller.startPairing()
        }
    }
    val name = customPeerName(app.peerName)
    val settingsLabel = stringResource(R.string.native_ui_common_settings)
    var choosing by remember { mutableStateOf(listRequested) }
    LaunchedEffect(listRequested) {
        if (listRequested) {
            choosing = true
            onListShown()
        }
    }
    val settings = controller.settings.collectAsState().value
    val roleTitle =
        stringResource(
            if (isParent) R.string.native_ui_role_parent_title
            else R.string.native_ui_role_baby_title
        )
    ScreenRoot("home") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Title(
                    stringResource(R.string.native_ui_common_app_name),
                    "home_title",
                    Modifier.weight(1f),
                )
                // The gear sits in the header; the whole 48 dp square is the target.
                GlyphButton(Glyph.GEAR, settingsLabel, onSettings, "home_settings")
            }
            if (app.paired) {
                // Role as a section header, the connection directly under it on one card.
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        roleTitle,
                        Modifier.testTag("home_role_header").semantics { heading() },
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onBackground,
                    )
                    Body(
                        stringResource(
                            if (isParent) R.string.native_ui_role_parent_hint
                            else R.string.native_ui_role_baby_hint
                        )
                    )
                }
                val selectedConnection = app.connections.firstOrNull { it.selected }
                val cardTitle =
                    selectedConnection?.name
                        ?: name
                        ?: if (selectedConnection != null && app.connections.size > 1)
                            connectionLabel(selectedConnection, app.connections, app.role)
                        else peerDisplayName(app.peerName, parentSide = isParent)
                AppCard(
                    Modifier.testTag("home_card").heightIn(min = 96.dp),
                    container = MaterialTheme.colorScheme.primaryContainer,
                    onClick = { controller.startMonitoring() },
                ) {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = 64.dp),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        GlyphIcon(
                            if (isParent) Glyph.HEADPHONES else Glyph.MIC,
                            MaterialTheme.colorScheme.onPrimaryContainer,
                            size = 36.dp,
                        )
                        Column(Modifier.weight(1f)) {
                            Text(
                                cardTitle,
                                Modifier.testTag("home_card_title"),
                                style = MaterialTheme.typography.headlineSmall,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onPrimaryContainer,
                            )
                            val last = app.lastUsedWallMs
                            if (last != null) {
                                Text(
                                    stringResource(
                                        R.string.native_ui_home_last_used,
                                        formatTime(last),
                                    ),
                                    Modifier.testTag("home_card_last"),
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                                )
                            }
                        }
                        // One verb everywhere: the pill names what a tap does.
                        Surface(
                            Modifier.testTag("home_start"),
                            shape = RoundedCornerShape(50),
                            color = MaterialTheme.colorScheme.primary,
                        ) {
                            Text(
                                stringResource(R.string.native_ui_home_start),
                                Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                                style = MaterialTheme.typography.titleMedium,
                                color = MaterialTheme.colorScheme.onPrimary,
                            )
                        }
                    }
                }
                // Only with more than one saved baby unit: the default is preselected for this
                // session, "Change" opens the list. One saved pairing shows no list.
                if (isParent && app.connections.size > 1) {
                    val current = app.connections.firstOrNull { it.selected }
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (current != null) {
                            Body(
                                stringResource(
                                    R.string.native_ui_connections_current,
                                    connectionLabel(current, app.connections, app.role),
                                ),
                                tag = "home_connection_current",
                                color = MaterialTheme.colorScheme.onBackground,
                            )
                        }
                        SecondaryButton(
                            stringResource(R.string.native_ui_connections_change),
                            { choosing = !choosing },
                            "home_connection_change",
                            Modifier.fillMaxWidth(),
                        )
                        if (choosing) {
                            AppCard(Modifier.testTag("home_connection_list")) {
                                app.connections.forEach { c ->
                                    ChoiceRow(
                                        connectionListLabel(c, app.connections, app.role),
                                        c.selected,
                                        "connection_choice_${c.id}",
                                    ) {
                                        controller.selectConnection(c.id)
                                        choosing = false
                                    }
                                }
                            }
                        }
                    }
                }
                // The marked default connects at app start; the switch binds to the selected one.
                val selected = app.connections.firstOrNull { it.selected }
                if (isParent && selected != null) {
                    SwitchRow(
                        stringResource(R.string.native_ui_connections_auto_connect),
                        selected.isDefault,
                        "home_auto_connect",
                    ) { on ->
                        if (on) controller.setDefaultConnection(selected.id)
                        else controller.clearDefaultConnection()
                    }
                }
                // `parent-alerts` "Vibrate-only reminder at start": a line, never a prompt.
                if (isParent && settings.alertStyle == AlertStyle.DISCREET) {
                    Body(
                        stringResource(R.string.native_ui_home_vibrate_only),
                        tag = "home_vibrate_only",
                        color = LocalAppColors.current.warn,
                    )
                }
                if (app.canAddPairing) {
                    PrimaryButton(
                        stringResource(R.string.native_ui_home_new_pairing),
                        { controller.startPairing() },
                        "home_new_pairing",
                        Modifier.fillMaxWidth(),
                        container = MaterialTheme.colorScheme.secondaryContainer,
                        content = MaterialTheme.colorScheme.onSecondaryContainer,
                    )
                }
            } else {
                Body(stringResource(R.string.native_ui_home_empty), tag = "home_empty")
                PrimaryButton(
                    stringResource(R.string.native_ui_home_new_pairing),
                    { controller.startPairing() },
                    "home_new_pairing",
                )
            }
            SwitchModeButton(controller, Modifier.fillMaxWidth())
            PrivacyLine("home_privacy")
        }
    }
}

/** Custom unit name, or null so copy can use the grammatically correct "the baby unit" form. */
internal fun customPeerName(peerName: String?): String? = peerName?.takeIf { it.isNotBlank() }

@Composable
internal fun peerDisplayName(peerName: String?, parentSide: Boolean): String =
    customPeerName(peerName)
        ?: stringResource(
            if (parentSide) R.string.native_ui_common_baby_unit_default
            else R.string.native_ui_common_parent_unit_default
        )
