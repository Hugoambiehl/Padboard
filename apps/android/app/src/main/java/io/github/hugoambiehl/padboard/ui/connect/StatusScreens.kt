package io.github.hugoambiehl.padboard.ui.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType

/**
 * Attempts during which the search indicator spins (about 20 s with the
 * backoff). After that it stands still: a PC turned off for the night must
 * not keep the tablet drawing 60 frames a second.
 */
const val SPINNING_ATTEMPTS = 6

/** Spinning while a search is young, a still ring afterwards. */
@Composable
fun SearchIndicator(attempt: Int, modifier: Modifier = Modifier) {
    if (attempt <= SPINNING_ATTEMPTS) {
        CircularProgressIndicator(modifier.size(24.dp), color = PadboardColors.Accent, strokeWidth = 3.dp)
    } else {
        CircularProgressIndicator(
            progress = { 1f },
            modifier = modifier.size(24.dp),
            color = PadboardColors.TextMuted,
            strokeWidth = 3.dp,
            trackColor = PadboardColors.TextMuted
        )
    }
}

/** Looking for the paired PC (no layout received yet). [serverName] null: starting up. */
@Composable
fun SearchingScreen(serverName: String?, onForget: (() -> Unit)?, attempt: Int = 0) {
    var confirm by rememberSaveable { mutableStateOf(false) }
    CenteredColumn {
        AppBadge()
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            SearchIndicator(attempt)
            Text(
                serverName?.let { stringResource(R.string.searching_title, it) } ?: stringResource(R.string.starting),
                style = PadboardType.CardTitle,
                color = PadboardColors.Text
            )
        }
        if (serverName != null) {
            Text(
                stringResource(R.string.searching_body),
                style = PadboardType.Body,
                color = PadboardColors.TextMuted2,
                modifier = Modifier.widthIn(max = 560.dp)
            )
        }
        if (onForget != null) {
            TextButton(onClick = { confirm = true }) {
                Text(stringResource(R.string.forget_button), color = PadboardColors.Accent)
            }
        }
    }
    if (confirm && onForget != null) {
        ForgetDialog(serverName.orEmpty(), onConfirm = onForget, onDismiss = { confirm = false })
    }
}

@Composable
fun IncompatibleScreen(onRetry: () -> Unit) {
    CenteredColumn {
        AppBadge()
        Text(
            stringResource(R.string.failure_incompatible),
            style = PadboardType.Body,
            color = PadboardColors.Text,
            modifier = Modifier.widthIn(max = 560.dp)
        )
        Button(
            onClick = onRetry,
            colors = ButtonDefaults.buttonColors(
                containerColor = PadboardColors.Accent,
                contentColor = PadboardColors.Background
            )
        ) {
            Text(stringResource(R.string.retry))
        }
    }
}

/** Confirmation before forgetting the paired PC (its token and pinned key). */
@Composable
fun ForgetDialog(serverName: String, onConfirm: () -> Unit, onDismiss: () -> Unit, onAbout: (() -> Unit)? = null) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = PadboardColors.Surface2,
        title = { Text(stringResource(R.string.forget_title, serverName), color = PadboardColors.Text) },
        text = { Text(stringResource(R.string.forget_body), color = PadboardColors.TextMuted2) },
        confirmButton = {
            TextButton(onClick = {
                onDismiss()
                onConfirm()
            }) { Text(stringResource(R.string.forget_confirm), color = PadboardColors.Red) }
        },
        dismissButton = {
            Row {
                if (onAbout != null) AboutButton(onDismiss, onAbout)
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel), color = PadboardColors.Text) }
            }
        }
    )
}

/** "À propos et licences" in a dialog: closes it, then opens the screen. */
@Composable
fun AboutButton(onDismiss: () -> Unit, onAbout: () -> Unit) {
    TextButton(onClick = {
        onDismiss()
        onAbout()
    }) { Text(stringResource(R.string.about_open), color = PadboardColors.TextMuted2) }
}

@Composable
private fun CenteredColumn(content: @Composable () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().background(PadboardColors.Background).padding(64.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.Start
    ) { content() }
}
