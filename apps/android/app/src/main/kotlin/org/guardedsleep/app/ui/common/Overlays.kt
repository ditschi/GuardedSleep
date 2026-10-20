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
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/*
 * Sheets and dialogs are drawn inside the screen (no separate window) so they are part of the
 * semantics tree, work with screenshot tests and can be dismissed with Back.
 */

@Composable
fun SheetOverlay(tag: String, title: String, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    BackHandler(onBack = onDismiss)
    val maxH = (LocalConfiguration.current.screenHeightDp * 0.85f).dp
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier.fillMaxSize().background(Color(0xAA000000))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onDismiss),
        )
        Surface(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().heightIn(max = maxH)
                .semantics { paneTitle = title }.testTag(tag),
            shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
            color = MaterialTheme.colorScheme.surface,
        ) {
            Column(
                Modifier.verticalScroll(rememberScrollState()).padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSurface)
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
            Modifier.fillMaxSize().background(Color(0xAA000000))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onDismiss),
        )
        Surface(
            Modifier.padding(24.dp).widthIn(max = 420.dp).semantics { paneTitle = title }.testTag(tag),
            shape = RoundedCornerShape(28.dp),
            color = MaterialTheme.colorScheme.surface,
        ) {
            Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSurface)
                Body(body)
                // Safe choice first and largest; the destructive one is smaller and apart.
                PrimaryButton(cancelText, onDismiss, cancelTag)
                SecondaryButton(confirmText, onConfirm, confirmTag, Modifier.fillMaxWidth(), destructive = destructive)
            }
        }
    }
}
