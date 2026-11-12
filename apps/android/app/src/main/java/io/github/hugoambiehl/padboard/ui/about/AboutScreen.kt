package io.github.hugoambiehl.padboard.ui.about

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.ui.connect.AppBadge
import io.github.hugoambiehl.padboard.ui.theme.PadboardColors
import io.github.hugoambiehl.padboard.ui.theme.PadboardType
import java.util.logging.Level
import java.util.logging.Logger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val SOURCE_URL = "https://github.com/Hugoambiehl/Padboard"
private val log = Logger.getLogger("About")

/**
 * "À propos et licences": the version, Padboard's license (GPL-3.0-or-later,
 * no warranty), where the source is, then every bundled component with its
 * license; a row opens to show the full text. Nothing is animated (texts are
 * only composed once opened).
 */
@Composable
fun AboutScreen(onBack: () -> Unit) {
    BackHandler(onBack = onBack)
    val context = LocalContext.current
    val libraries by produceState(emptyList<Component>()) {
        value =
            withContext(Dispatchers.IO) { loadLibraries(context) }
    }
    val open = remember { mutableStateListOf<String>() }

    Row(
        modifier =
        Modifier
            .fillMaxSize()
            .background(PadboardColors.Background)
            .padding(horizontal = 64.dp, vertical = 48.dp)
            .testTag("about"),
        horizontalArrangement = Arrangement.spacedBy(48.dp)
    ) {
        Column(modifier = Modifier.width(380.dp).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            AppBadge()
            Text(stringResource(R.string.about_title), style = PadboardType.Display, color = PadboardColors.Text)
            Text(
                stringResource(R.string.about_version, versionName(context)),
                style = PadboardType.Body,
                color = PadboardColors.TextMuted2
            )
            Text(stringResource(R.string.about_license), style = PadboardType.Body, color = PadboardColors.Text)
            Text(stringResource(R.string.about_warranty), style = PadboardType.Small, color = PadboardColors.TextMuted)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    stringResource(R.string.about_source),
                    style = PadboardType.Small,
                    color = PadboardColors.TextMuted
                )
                Text(SOURCE_URL, style = PadboardType.Small, color = PadboardColors.TextMuted2)
            }
            TextButton(onClick = onBack) { Text(stringResource(R.string.about_back), color = PadboardColors.Accent) }
        }
        LazyColumn(
            modifier = Modifier.weight(1f).fillMaxHeight().testTag("about-components"),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            val sections =
                listOf(
                    R.string.about_section_app to listOf(PADBOARD),
                    R.string.about_section_assets to BUNDLED,
                    R.string.about_section_libraries to libraries
                )
            for ((title, components) in sections) {
                item(key = "title-$title") {
                    Text(
                        stringResource(title),
                        style = PadboardType.Badge,
                        color = PadboardColors.TextMuted,
                        modifier = Modifier.padding(top = 12.dp, bottom = 4.dp)
                    )
                }
                items(components, key = { it.name }) { component ->
                    ComponentRow(
                        component = component,
                        expanded = component.name in open,
                        onToggle = { if (!open.remove(component.name)) open.add(component.name) }
                    )
                }
            }
        }
    }
}

@Composable
private fun ComponentRow(component: Component, expanded: Boolean, onToggle: () -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    Column(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(PadboardColors.Surface2)
            .border(1.dp, PadboardColors.Border2, shape)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().clickable(
                onClick = onToggle
            ).padding(horizontal = 18.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                component.name,
                style = PadboardType.Pill,
                color = PadboardColors.Text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false)
            )
            Text(component.detail, style = PadboardType.Small, color = PadboardColors.TextMuted, maxLines = 1)
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.End) {
                Text(component.license, style = PadboardType.Small, color = PadboardColors.TextMuted2, maxLines = 1)
            }
            Icon(
                painterResource(R.drawable.ic_chevron_down),
                contentDescription = null,
                tint = PadboardColors.TextMuted,
                modifier = Modifier.size(18.dp).rotate(if (expanded) 180f else 0f)
            )
        }
        if (expanded) LicenseTexts(component)
    }
}

@Composable
private fun LicenseTexts(component: Component) {
    val context = LocalContext.current
    val texts by produceState(emptyList<String>(), component) {
        value = withContext(Dispatchers.IO) { component.texts.mapNotNull { readAsset(context, it) } }
    }
    Column(
        Modifier.padding(start = 18.dp, end = 18.dp, bottom = 16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        if (component.texts.isEmpty()) {
            Text(stringResource(R.string.about_no_text), style = PadboardType.Small, color = PadboardColors.TextMuted2)
        }
        for (text in texts) {
            Text(
                text,
                style = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 17.sp),
                color = PadboardColors.TextMuted2,
                modifier =
                Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .background(PadboardColors.Background)
                    .horizontalScroll(rememberScrollState())
                    .padding(12.dp)
            )
        }
    }
}

private fun versionName(context: Context): String = runCatching {
    context.packageManager.getPackageInfo(context.packageName, 0).versionName
}.getOrNull() ?: "?"

private fun readAsset(context: Context, name: String): String? = runCatching {
    context.assets.open("$LICENSES_DIR/$name").bufferedReader().use { it.readText().trim() }
}.onFailure { log.log(Level.WARNING, "license text $name unreadable", it) }.getOrNull()

private fun loadLibraries(context: Context): List<Component> = readAsset(context, LIBRARIES_FILE)
    ?.let {
        runCatching {
            parseLibraries(it)
        }.onFailure { e -> log.log(Level.WARNING, "libraries unreadable", e) }.getOrNull()
    }
    .orEmpty()
