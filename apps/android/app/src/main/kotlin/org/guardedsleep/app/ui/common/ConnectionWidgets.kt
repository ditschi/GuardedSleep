package org.guardedsleep.app.ui.common

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.guardedsleep.app.contract.R
import org.guardedsleep.app.ui.model.ConnectionUi
import org.guardedsleep.app.ui.model.Role

/** The name the user chose, else a numbered default so two unnamed connections differ. */
@Composable
internal fun connectionLabel(c: ConnectionUi, all: List<ConnectionUi>, role: Role?): String =
    c.name
        ?: stringResource(
            if (role == Role.BABY) R.string.native_ui_connections_default_parent
            else R.string.native_ui_connections_default_baby,
            all.indexOfFirst { it.id == c.id } + 1,
        )

/** The label of a connection in a list: its name, marked when it is the default. */
@Composable
internal fun connectionListLabel(c: ConnectionUi, all: List<ConnectionUi>, role: Role?): String {
    val label = connectionLabel(c, all, role)
    return if (c.isDefault) stringResource(R.string.native_ui_connections_label_default, label)
    else label
}

/** [text] cut to [ConnectionUi.MAX_NAME_LENGTH] code points (never inside a surrogate pair). */
internal fun limitName(text: String): String =
    if (text.codePointCount(0, text.length) <= ConnectionUi.MAX_NAME_LENGTH) text
    else text.substring(0, text.offsetByCodePoints(0, ConnectionUi.MAX_NAME_LENGTH))

/** Asks for a new local connection name; the name never leaves this phone. */
@Composable
internal fun RenameConnectionDialog(
    initial: String,
    onSave: (String) -> Boolean,
    onDismiss: () -> Unit,
) {
    var text by remember { mutableStateOf(initial) }
    var invalid by remember { mutableStateOf(false) }
    val title = stringResource(R.string.native_ui_connections_rename_title)
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
                .testTag("dialog_rename_connection"),
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
                OutlinedTextField(
                    value = text,
                    onValueChange = {
                        // Counted in code points like the store, so an emoji is not cut early.
                        text = limitName(it)
                        invalid = false
                    },
                    label = { Text(stringResource(R.string.native_ui_connections_name_label)) },
                    isError = invalid,
                    supportingText =
                        if (invalid) {
                            { Text(stringResource(R.string.native_ui_connections_name_invalid)) }
                        } else null,
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().testTag("rename_connection_field"),
                )
                PrimaryButton(
                    stringResource(R.string.native_ui_connections_save),
                    { if (onSave(text)) onDismiss() else invalid = true },
                    "rename_connection_save",
                )
                SecondaryButton(
                    stringResource(R.string.native_ui_common_cancel),
                    onDismiss,
                    "rename_connection_cancel",
                    Modifier.fillMaxWidth(),
                    destructive = false,
                )
            }
        }
    }
}
