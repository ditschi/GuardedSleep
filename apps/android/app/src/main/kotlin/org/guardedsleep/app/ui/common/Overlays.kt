package org.guardedsleep.app.ui.common

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R

/*
 * Sheets and dialogs are drawn inside the screen (no separate window) so they are part of the
 * semantics tree, work with screenshot tests and can be dismissed with Back.
 */

@Composable
fun SheetOverlay(
    tag: String,
    title: String,
    onDismiss: () -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    BackHandler(onBack = onDismiss)
    val windowHeight = LocalWindowInfo.current.containerSize.height
    val maxH = with(LocalDensity.current) { (windowHeight * 0.85f).toDp() }
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier.fillMaxSize()
                .background(Color(0xAA000000))
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = onDismiss,
                )
        )
        Surface(
            Modifier.align(Alignment.BottomCenter)
                .fillMaxWidth()
                .heightIn(max = maxH)
                .semantics { paneTitle = title }
                .testTag(tag),
            shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
            color = MaterialTheme.colorScheme.surface,
        ) {
            Column(
                Modifier.verticalScroll(rememberScrollState()).padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                content()
            }
        }
    }
}

@Composable
fun ConfirmDialog(
    tag: String,
    title: String,
    body: String,
    confirmText: String,
    cancelText: String,
    confirmTag: String,
    cancelTag: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    destructive: Boolean = true,
) {
    BackHandler(onBack = onDismiss)
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Box(
            Modifier.fillMaxSize()
                .background(Color(0xAA000000))
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = onDismiss,
                )
        )
        Surface(
            Modifier.padding(24.dp)
                .widthIn(max = 420.dp)
                .semantics { paneTitle = title }
                .testTag(tag),
            shape = RoundedCornerShape(28.dp),
            color = MaterialTheme.colorScheme.surface,
        ) {
            Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Body(body)
                // Safe choice first and largest; the destructive one is smaller and apart.
                PrimaryButton(cancelText, onDismiss, cancelTag)
                SecondaryButton(
                    confirmText,
                    onConfirm,
                    confirmTag,
                    Modifier.fillMaxWidth(),
                    destructive = destructive,
                )
            }
        }
    }
}

/**
 * The stop confirmation of `device-setup` "Stopping monitoring on the parent": one dialog for the
 * More sheet, the quiet Stop button and "Wrong connection?" of the alert screens. Opening or
 * cancelling it does nothing else: it silences and acknowledges no alert, so a running alarm keeps
 * sounding underneath. "Keep monitoring" is the filled default in the thumb position.
 */
@Composable
fun StopMonitoringDialog(
    name: String?,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    tag: String = "dialog_stop",
    confirmTag: String = "dialog_stop_confirm",
    cancelTag: String = "dialog_stop_cancel",
) {
    ConfirmDialog(
        tag = tag,
        title =
            if (name != null)
                stringResource(
                    R.string.native_ui_stop_title_named,
                    name,
                )
            else stringResource(R.string.native_ui_stop_title),
        body =
            if (name != null)
                stringResource(
                    R.string.native_ui_stop_parent_body,
                    name,
                )
            else stringResource(R.string.native_ui_stop_parent_body_default),
        confirmText = stringResource(R.string.native_ui_stop_confirm),
        cancelText = stringResource(R.string.native_ui_stop_cancel),
        confirmTag = confirmTag,
        cancelTag = cancelTag,
        onConfirm = onConfirm,
        onDismiss = onDismiss,
    )
}
