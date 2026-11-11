package io.github.hugoambiehl.padboard.ui.grid

import io.github.hugoambiehl.padboard.protocol.Message
import io.github.hugoambiehl.padboard.protocol.parseMessage
import java.io.File
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

class IconVectorsTest {
    private val fixtures = File(System.getProperty("padboard.fixtures") ?: error("padboard.fixtures is not set"))

    @Test
    fun iconsOfTheSharedFixtureAreDrawable() {
        val layout = (parseMessage(File(fixtures, "layout_sync.json").readText()) as Message.LayoutSync).payload
        val icons = layout.pages.flatMap { it.buttons }.mapNotNull { it.icon }
        check(icons.isNotEmpty()) { "the fixture should carry an icon" }
        for (icon in icons) assertNotNull(icon.paths.toString(), IconVectors.build(icon.paths))
    }

    @Test
    fun theSamePathsGiveTheSameVector() {
        val paths = listOf("M5 12h14", "M12 5v14")
        assertSame(IconVectors.get(paths), IconVectors.get(paths.toList()))
    }

    @Test
    fun noPathsMeansNoIcon() {
        assertNull(IconVectors.build(emptyList()))
    }
}
