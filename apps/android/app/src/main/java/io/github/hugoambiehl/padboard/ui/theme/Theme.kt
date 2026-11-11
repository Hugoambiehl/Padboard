package io.github.hugoambiehl.padboard.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import io.github.hugoambiehl.padboard.R
import io.github.hugoambiehl.padboard.protocol.Category

/** Design tokens (docs/design, CLAUDE.md). Dark theme only. */
object PadboardColors {
    val Background = Color(0xFF0E0F12)
    val Surface = Color(0xFF121317)
    val Surface2 = Color(0xFF17181D)
    val SurfaceActive = Color(0xFF1E2029)
    val Border = Color(0xFF22242B)
    val Border2 = Color(0xFF24262E)
    val Text = Color(0xFFE8EAF0)
    val TextPill = Color(0xFFC3C8D4)
    val TextMuted = Color(0xFF8A90A0)
    val TextMuted2 = Color(0xFFA3ABBD)
    val Accent = Color(0xFF8B7CFF)
    val DotInactive = Color(0xFF3A3D47)

    val Violet = Color(0xFF8B7CFF)
    val Blue = Color(0xFF5AA9FF)
    val Teal = Color(0xFF3DD6B5)
    val Orange = Color(0xFFFF9F5A)
    val Pink = Color(0xFFFF6FAE)
    val Red = Color(0xFFFF5C5C)
    val Grey = Color(0xFFA3ABBD)
}

/** Icon color and its tinted background, as in Main.dc.html (16 %, red 18 %, grey 14 %). */
data class CategoryColors(val foreground: Color, val background: Color)

fun Category.colors(): CategoryColors = when (this) {
    Category.Violet -> CategoryColors(PadboardColors.Violet, PadboardColors.Violet.copy(alpha = 0.16f))
    Category.Blue -> CategoryColors(PadboardColors.Blue, PadboardColors.Blue.copy(alpha = 0.16f))
    Category.Teal -> CategoryColors(PadboardColors.Teal, PadboardColors.Teal.copy(alpha = 0.16f))
    Category.Orange -> CategoryColors(PadboardColors.Orange, PadboardColors.Orange.copy(alpha = 0.16f))
    Category.Pink -> CategoryColors(PadboardColors.Pink, PadboardColors.Pink.copy(alpha = 0.16f))
    Category.Red -> CategoryColors(PadboardColors.Red, PadboardColors.Red.copy(alpha = 0.18f))
    Category.Grey -> CategoryColors(PadboardColors.Grey, PadboardColors.Grey.copy(alpha = 0.14f))
}

/** Inter, bundled (res/font). */
val Inter =
    FontFamily(
        Font(R.font.inter_regular, FontWeight.Normal),
        Font(R.font.inter_medium, FontWeight.Medium),
        Font(R.font.inter_semibold, FontWeight.SemiBold)
    )

/** Space Grotesk, bundled (res/font), for titles. */
val SpaceGrotesk =
    FontFamily(
        Font(R.font.space_grotesk_medium, FontWeight.Medium),
        Font(R.font.space_grotesk_semibold, FontWeight.SemiBold)
    )

private val ColorScheme =
    darkColorScheme(
        primary = PadboardColors.Accent,
        onPrimary = PadboardColors.Background,
        background = PadboardColors.Background,
        onBackground = PadboardColors.Text,
        surface = PadboardColors.Surface2,
        onSurface = PadboardColors.Text,
        onSurfaceVariant = PadboardColors.TextMuted,
        outline = PadboardColors.Border2,
        error = PadboardColors.Red
    )

@Composable
fun PadboardTheme(content: @Composable () -> Unit) {
    val base = MaterialTheme.typography
    val typography =
        base.copy(
            bodyLarge = base.bodyLarge.copy(fontFamily = Inter),
            bodyMedium = base.bodyMedium.copy(fontFamily = Inter),
            bodySmall = base.bodySmall.copy(fontFamily = Inter),
            labelLarge = base.labelLarge.copy(fontFamily = Inter),
            labelMedium = base.labelMedium.copy(fontFamily = Inter),
            labelSmall = base.labelSmall.copy(fontFamily = Inter),
            titleLarge = base.titleLarge.copy(fontFamily = SpaceGrotesk),
            titleMedium = base.titleMedium.copy(fontFamily = Inter),
            headlineLarge = base.headlineLarge.copy(fontFamily = SpaceGrotesk)
        )
    MaterialTheme(colorScheme = ColorScheme, typography = typography, content = content)
}

/** Text styles used by the mockups, in sp. */
object PadboardType {
    val Display =
        TextStyle(fontFamily = SpaceGrotesk, fontWeight = FontWeight.SemiBold, fontSize = 44.sp, lineHeight = 48.sp)
    val CardTitle = TextStyle(fontFamily = SpaceGrotesk, fontWeight = FontWeight.SemiBold, fontSize = 24.sp)
    val Profile = TextStyle(fontFamily = SpaceGrotesk, fontWeight = FontWeight.SemiBold, fontSize = 17.sp)
    val Clock = TextStyle(fontFamily = SpaceGrotesk, fontWeight = FontWeight.Medium, fontSize = 22.sp)
    val Initial = TextStyle(fontFamily = SpaceGrotesk, fontWeight = FontWeight.SemiBold, fontSize = 26.sp)
    val Body = TextStyle(fontFamily = Inter, fontSize = 17.sp, lineHeight = 26.sp)
    val TileLabel = TextStyle(fontFamily = Inter, fontWeight = FontWeight.SemiBold, fontSize = 17.sp)
    val TileSubtitle = TextStyle(fontFamily = Inter, fontSize = 13.sp)
    val Pill = TextStyle(fontFamily = Inter, fontSize = 14.sp)
    val Badge =
        TextStyle(fontFamily = Inter, fontWeight = FontWeight.SemiBold, fontSize = 12.sp, letterSpacing = 0.48.sp)
    val Small = TextStyle(fontFamily = Inter, fontSize = 14.sp, lineHeight = 20.sp)
}
