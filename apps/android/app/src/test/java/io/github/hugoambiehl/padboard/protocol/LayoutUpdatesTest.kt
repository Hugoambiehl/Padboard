package io.github.hugoambiehl.padboard.protocol

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class LayoutUpdatesTest {
    private fun button(id: String, label: String = id) =
        ButtonView(id, 0u, 0u, label, category = Category.Grey, active = false, unavailable = false)

    private val layout =
        Layout(
            ProfileInfo("p", "P"),
            GridSize(5u, 3u),
            listOf(Page("p1", listOf(button("a"))), Page("p2", listOf(button("b")))),
            listOf(Folder("f", "F", listOf(button("c"))))
        )

    @Test
    fun aButtonIsReplacedInAPage() {
        val next = layout.withButton(button("b", "B").copy(active = true))
        assertTrue(next.pages[1].buttons[0].active)
        assertEquals("B", next.pages[1].buttons[0].label)
        // The rest is shared, not copied.
        assertSame(layout.pages[0], next.pages[0])
        assertSame(layout.folders, next.folders)
    }

    @Test
    fun aButtonIsReplacedInAFolder() {
        val next = layout.withButton(button("c").copy(unavailable = true))
        assertTrue(next.folders[0].buttons[0].unavailable)
        assertSame(layout.pages, next.pages)
    }

    @Test
    fun unknownOrUnchangedButtonsKeepTheSameLayout() {
        assertSame(layout, layout.withButton(button("zzz")))
        assertSame(layout, layout.withButton(button("a")))
    }
}
