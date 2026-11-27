package io.github.hugoambiehl.padboard.ui.connect

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.connection.LocalNetwork
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType

/**
 * Asks Android 17 for the local network (`LocalNetwork`). Once refused for
 * good (Android no longer shows its dialog), opens the app's settings
 * instead, where "Nearby devices" can be allowed.
 */
@Composable
fun rememberLocalNetworkRequest(): () -> Unit {
    val context = LocalContext.current
    var refused by rememberSaveable { mutableStateOf(false) }
    val launcher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            LocalNetwork.refresh(context)
            val activity = context.findActivity()
            if (!granted && activity?.shouldShowRequestPermissionRationale(LocalNetwork.PERMISSION) == false) {
                refused = true
            }
        }
    return {
        if (refused) {
            context.startActivity(
                Intent(
                    Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                    Uri.fromParts("package", context.packageName, null)
                )
            )
        } else {
            launcher.launch(LocalNetwork.PERMISSION)
        }
    }
}

/** Why Wi-Fi is off on Android 17, and the button that asks for it. */
@Composable
fun LocalNetworkNotice(onAllow: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.widthIn(max = 560.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.local_network_title), style = PadboardType.CardTitle, color = PadboardColors.Text)
        Text(stringResource(R.string.local_network_body), style = PadboardType.Body, color = PadboardColors.TextMuted2)
        LocalNetworkButton(onAllow)
    }
}

@Composable
fun LocalNetworkButton(onAllow: () -> Unit) {
    Button(
        onClick = onAllow,
        colors = ButtonDefaults.buttonColors(
            containerColor = PadboardColors.Accent,
            contentColor = PadboardColors.Background
        )
    ) { Text(stringResource(R.string.local_network_allow)) }
}

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}
