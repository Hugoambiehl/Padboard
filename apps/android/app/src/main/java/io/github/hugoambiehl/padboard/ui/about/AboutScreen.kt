package io.github.hugoambiehl.padboard.ui.about

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.RectangleShape
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
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val SOURCE_URL = "https://github.com/Hugoambiehl/Padboard"
private val log = Logger.getLogger("About")

/**
 * "À propos et licences": the version, Padboard's license (GPL-3.0-or-later,
 * no warranty), where the source is, then every bundled component with its
 * license; a row opens to show the full text, one list item per paragraph.
 * Nothing is animated.
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
    val paragraphs = remember { mutableStateMapOf<String, List<String>>() }
    val scope = rememberCoroutineScope()

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
        LazyColumn(modifier = Modifier.weight(1f).fillMaxHeight().testTag("about-components")) {
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
                        modifier = Modifier.padding(top = 20.dp, bottom = 4.dp)
                    )
                }
                for (component in components) {
                    val expanded = component.name in open
                    item(key = component.name) {
                        ComponentHeader(component, expanded) {
                            if (!open.remove(component.name)) {
                                open.add(component.name)
                                if (component.name !in paragraphs) {
                                    scope.launch {
                                        paragraphs[component.name] = withContext(Dispatchers.IO) {
                                            licenseParagraphs(context, component)
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if (expanded) {
                        // One item per paragraph: a long text (the GPL) is only drawn where visible.
                        val parts = paragraphs[component.name].orEmpty()
                        itemsIndexed(parts, key = { i, _ -> "${component.name}#$i" }) { i, part ->
                            Paragraph(part, last = i == parts.lastIndex)
                        }
                    }
                }
            }
        }
    }
}

private val ROW = 16.dp

@Composable
private fun ComponentHeader(component: Component, expanded: Boolean, onToggle: () -> Unit) {
    val shape =
        if (expanded) RoundedCornerShape(topStart = ROW, topEnd = ROW) else RoundedCornerShape(ROW)
    Row(
        modifier =
        Modifier
            .padding(top = 8.dp)
            .fillMaxWidth()
            .clip(shape)
            .background(PadboardColors.Surface2)
            .clickable(onClick = onToggle)
            .padding(horizontal = 18.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Name and version on the left (the name shortened first), license on the right.
        Row(
            Modifier.weight(1f),
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
        }
        Text(component.license, style = PadboardType.Small, color = PadboardColors.TextMuted2, maxLines = 1)
        Icon(
            painterResource(R.drawable.ic_chevron_down),
            contentDescription = null,
            tint = PadboardColors.TextMuted,
            modifier = Modifier.size(18.dp).rotate(if (expanded) 180f else 0f)
        )
    }
}

private val MONO = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 17.sp)

@Composable
private fun Paragraph(text: String, last: Boolean) {
    val shape = if (last) RoundedCornerShape(bottomStart = ROW, bottomEnd = ROW) else RectangleShape
    Text(
        text.ifEmpty { stringResource(R.string.about_no_text) },
        style = if (text.isEmpty()) PadboardType.Small else MONO,
        color = PadboardColors.TextMuted2,
        modifier =
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(PadboardColors.Surface2)
            .padding(start = 18.dp, end = 18.dp, bottom = if (last) 16.dp else 10.dp)
    )
}

/** The component's license texts split into paragraphs ("" when it bundles none). */
private fun licenseParagraphs(context: Context, component: Component): List<String> {
    if (component.texts.isEmpty()) return listOf("")
    return component.texts
        .mapNotNull { readAsset(context, it) }
        .flatMap { it.split(Regex("\n[ \t]*\n")) }
        .map { it.trimEnd() }
        .filter { it.isNotBlank() }
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
