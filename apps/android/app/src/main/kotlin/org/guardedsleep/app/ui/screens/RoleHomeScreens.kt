package org.guardedsleep.app.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.common.AppCard
import org.guardedsleep.app.ui.common.Body
import org.guardedsleep.app.ui.common.Glyph
import org.guardedsleep.app.ui.common.GlyphIcon
import org.guardedsleep.app.ui.common.PrimaryButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.connectionLabel
import org.guardedsleep.app.ui.common.connectionListLabel
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.Role

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
        }
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
fun HomeScreen(controller: AppController, onSettings: () -> Unit) {
    val app = controller.app.collectAsState().value
    val isParent = app.role == Role.PARENT
    val name = customPeerName(app.peerName)
    var choosing by remember { mutableStateOf(false) }
    ScreenRoot("home") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Title(
                    stringResource(R.string.native_ui_common_app_name),
                    "home_title",
                    Modifier.weight(1f),
                )
            }
            if (app.paired) {
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
                                if (isParent)
                                    (if (name != null)
                                        stringResource(
                                            R.string.native_ui_home_parent_card_title,
                                            name,
                                        )
                                    else
                                        stringResource(
                                            R.string.native_ui_home_parent_card_title_default
                                        ))
                                else stringResource(R.string.native_ui_home_baby_card_title),
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
                if (app.canAddPairing) {
                    SecondaryButton(
                        stringResource(R.string.native_ui_home_new_pairing),
                        { controller.startPairing() },
                        "home_new_pairing",
                        Modifier.fillMaxWidth(),
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
            SecondaryButton(
                stringResource(R.string.native_ui_common_settings),
                onSettings,
                "home_settings",
                Modifier.fillMaxWidth(),
            )
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
