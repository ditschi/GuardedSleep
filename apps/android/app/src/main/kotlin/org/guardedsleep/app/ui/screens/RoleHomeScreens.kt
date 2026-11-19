package org.guardedsleep.app.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role as SemanticsRole
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
import org.guardedsleep.app.ui.common.QuietButton
import org.guardedsleep.app.ui.common.ScreenRoot
import org.guardedsleep.app.ui.common.ScrollColumn
import org.guardedsleep.app.ui.common.SecondaryButton
import org.guardedsleep.app.ui.common.Title
import org.guardedsleep.app.ui.common.connectionLabel
import org.guardedsleep.app.ui.common.formatTime
import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.AppController
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.Role
import org.guardedsleep.app.ui.theme.LocalAppColors

/**
 * S01: the first start only, with no pairing in either role. Two large choices (120 dp each); the
 * start mode is not set here but in Settings › Open at start.
 */
@Composable
fun RoleScreen(controller: AppController) {
    ScreenRoot("role") {
        ScrollColumn(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Title(stringResource(R.string.native_ui_role_title), "role_title")
            RoleCard(
                "role_baby",
                Glyph.MIC,
                R.string.native_ui_role_baby_title,
                R.string.native_ui_role_baby_hint,
            ) {
                controller.chooseRole(Role.BABY)
            }
            RoleCard(
                "role_parent",
                Glyph.BELL,
                R.string.native_ui_role_parent_title,
                R.string.native_ui_role_parent_hint,
            ) {
                controller.chooseRole(Role.PARENT)
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

/**
 * S06: one Home with both roles (`device-setup` "Role choice"). Each role has its saved connections
 * with one Start each, and its own "Set up new devices". Start picks the role and the pairing; the
 * start mode and the automatic start live in Settings › Open at start, reached by the text button
 * at the end.
 */
@Composable
fun HomeScreen(
    controller: AppController,
    onSettings: () -> Unit,
    onOpenAtStart: () -> Unit = onSettings,
    /** True once the parent's empty home opened the scan screen for this entry. */
    autoScanned: Boolean = true,
    onAutoScanned: () -> Unit = {},
) {
    val app = controller.app.collectAsState().value
    val settings = controller.settings.collectAsState().value
    // `device-setup`, "Parent pairing entry": with no parent pairing saved, the parent role goes
    // straight to the scan screen, once. Cancelling lands here again; the flag keeps it from
    // reopening.
    LaunchedEffect(app.role, app.parentConnections.isEmpty(), autoScanned) {
        if (
            app.role == Role.PARENT &&
                app.parentConnections.isEmpty() &&
                app.canAddParentPairing &&
                !autoScanned
        ) {
            onAutoScanned()
            controller.startPairing()
        }
    }
    val settingsLabel = stringResource(R.string.native_ui_common_settings)
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
            HomeRole(
                controller,
                Role.PARENT,
                app.parentConnections,
                canAdd = app.canAddParentPairing,
                vibrateOnly = settings.alertStyle == AlertStyle.DISCREET,
                lastUsedWallMs = app.lastUsedWallMs.takeIf { app.lastRole == Role.PARENT },
            )
            HomeRole(
                controller,
                Role.BABY,
                app.babyConnections,
                canAdd = true,
                lastUsedWallMs = app.lastUsedWallMs.takeIf { app.lastRole == Role.BABY },
            )
            QuietButton(
                stringResource(
                    R.string.native_ui_home_open_at_start,
                    startModeLabel(app.defaultRole),
                ),
                onOpenAtStart,
                "home_open_at_start",
                Modifier.fillMaxWidth(),
            )
            PrivacyLine("home_privacy")
        }
    }
}

/** The one-word start mode: the role a start opens in, or "Ask every time". */
@Composable
internal fun startModeLabel(mode: Role?): String =
    stringResource(
        when (mode) {
            null -> R.string.native_ui_settings_start_mode_ask
            Role.BABY -> R.string.native_ui_role_baby_title
            Role.PARENT -> R.string.native_ui_role_parent_title
        }
    )

/** A role as a section header, its connections on one card, then its "Set up new devices". */
@Composable
private fun HomeRole(
    controller: AppController,
    role: Role,
    connections: List<ConnectionUi>,
    canAdd: Boolean,
    lastUsedWallMs: Long?,
    /** `parent-alerts` "Vibrate-only reminder at start": a line under the connections. */
    vibrateOnly: Boolean = false,
) {
    val key = role.name.lowercase()
    val isParent = role == Role.PARENT
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            stringResource(
                if (isParent) R.string.native_ui_role_parent_title
                else R.string.native_ui_role_baby_title
            ),
            Modifier.testTag("home_role_header_$key").semantics { heading() },
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
    if (connections.isEmpty()) {
        Body(stringResource(R.string.native_ui_home_empty), tag = "home_empty_$key")
    } else {
        AppCard(Modifier.testTag("home_connections_$key")) {
            connections.forEach { c ->
                ConnectionRow(
                    role,
                    c,
                    connectionLabel(c, connections, role),
                    lastUsedWallMs.takeIf { c.selected },
                ) {
                    controller.startConnection(role, c.id)
                }
            }
        }
    }
    if (vibrateOnly && connections.isNotEmpty()) {
        Body(
            stringResource(R.string.native_ui_home_vibrate_only),
            tag = "home_vibrate_only",
            color = LocalAppColors.current.warn,
        )
    }
    if (canAdd) {
        SecondaryButton(
            stringResource(R.string.native_ui_home_new_pairing),
            { controller.setUpNewDevices(role) },
            "home_new_pairing_$key",
            Modifier.fillMaxWidth(),
        )
    }
}

/** One saved connection: its name and a Start, one verb everywhere; the whole row starts it. */
@Composable
private fun ConnectionRow(
    role: Role,
    c: ConnectionUi,
    label: String,
    lastUsedWallMs: Long?,
    onStart: () -> Unit,
) {
    val key = role.name.lowercase()
    val startLabel = stringResource(R.string.native_ui_home_start_connection, label)
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 64.dp)
            .clickable(role = SemanticsRole.Button, onClick = onStart)
            .semantics(mergeDescendants = true) { contentDescription = startLabel }
            .testTag("home_connection_${key}_${c.id}"),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                label,
                Modifier.testTag("home_connection_name_${key}_${c.id}"),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (lastUsedWallMs != null) {
                Body(
                    stringResource(R.string.native_ui_home_last_used, formatTime(lastUsedWallMs)),
                    tag = "home_connection_last_${key}_${c.id}",
                )
            }
        }
        // The pill names what a tap does; the selected connection is the filled one.
        Surface(
            Modifier.testTag("home_start_${key}_${c.id}"),
            shape = RoundedCornerShape(50),
            color =
                if (c.selected) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.secondaryContainer,
        ) {
            Text(
                stringResource(R.string.native_ui_home_start),
                Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                style = MaterialTheme.typography.titleMedium,
                color =
                    if (c.selected) MaterialTheme.colorScheme.onPrimary
                    else MaterialTheme.colorScheme.onSecondaryContainer,
            )
        }
    }
}
