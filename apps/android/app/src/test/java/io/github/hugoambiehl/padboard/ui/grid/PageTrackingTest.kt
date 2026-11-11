package io.github.hugoambiehl.padboard.ui.grid

import io.github.hugoambiehl.padboard.protocol.Folder
import io.github.hugoambiehl.padboard.protocol.GridSize
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.NavigateTarget
import io.github.hugoambiehl.padboard.protocol.Page
import io.github.hugoambiehl.padboard.protocol.ProfileInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PageTrackingTest {
    private fun layout(profile: String, vararg pages: String, folders: List<String> = listOf("f1", "f2")) = Layout(
        profile = ProfileInfo(profile, profile),
        grid = GridSize(5u, 3u),
        pages = pages.map { Page(it, emptyList()) },
        folders = folders.map { Folder(it, it.uppercase(), emptyList()) }
    )

    private fun nav(to: NavigateTarget, id: String? = null) = NavigatePayload(to, id)

    /** A memory where the user is on `page` of `layout`. */
    private fun on(layout: Layout, page: String) = PageMemory().apply {
        onShown(layout, layout.pages.indexOfFirst { it.id == page })
    }

    @Test
    fun nothingShownYetOrAnotherProfileStartsOnTheFirstPage() {
        assertEquals(0, PageMemory().pageFor(layout("a", "p1", "p2")))
        assertEquals(0, on(layout("a", "p1", "p2"), "p2").pageFor(layout("b", "p1", "p2")))
    }

    @Test
    fun theShownPageIsFollowedByIdWhenPagesMove() {
        val memory = on(layout("a", "p1", "p2", "p3"), "p2")
        assertEquals(1, memory.pageFor(layout("a", "p1", "p2", "p3")))
        assertEquals(2, memory.pageFor(layout("a", "p0", "p1", "p2", "p3")))
        assertEquals(0, memory.pageFor(layout("a", "p2", "p1", "p3")))
    }

    @Test
    fun aPageDeletedLiveGivesWayToThePageAtItsPosition() {
        assertEquals(1, on(layout("a", "p1", "p2", "p3"), "p2").pageFor(layout("a", "p1", "p3")))
        // It was the last one: the new last page.
        assertEquals(1, on(layout("a", "p1", "p2", "p3"), "p3").pageFor(layout("a", "p1", "p2")))
    }

    @Test
    fun aReconnectionKeepsThePageWhenItStillExists() {
        val before = layout("a", "p1", "p2", "p3")
        val memory = on(before, "p3")
        memory.onConnectionLost()
        // Same layout sent again by the new connection (USB or Wi-Fi).
        assertEquals(2, memory.pageFor(layout("a", "p1", "p2", "p3")))
        // Pages edited while disconnected: still followed by id.
        assertEquals(0, memory.pageFor(layout("a", "p3", "p1")))
    }

    @Test
    fun aReconnectionGoesBackToTheFirstPageWhenThePageOrProfileIsGone() {
        val memory = on(layout("a", "p1", "p2", "p3"), "p2")
        memory.onConnectionLost()
        assertEquals(0, memory.pageFor(layout("a", "p1", "p3")))
        assertEquals(0, memory.pageFor(layout("b", "p1", "p2")))
    }

    @Test
    fun aNewShownPageAfterReconnectingHasItsOwnFallback() {
        val memory = on(layout("a", "p1", "p2", "p3"), "p2")
        memory.onConnectionLost()
        val after = layout("a", "p1", "p2", "p3")
        memory.onShown(after, memory.pageFor(after))
        // Deleted live again: the neighbour, not the first page.
        assertEquals(1, memory.pageFor(layout("a", "p1", "p3")))
    }

    @Test
    fun foldersOpenOnTopAndCloseWithBack() {
        val l = layout("a", "p1", "p2")
        val memory = PageMemory()
        assertNull(memory.folderFor(l))
        assertNull(memory.navigate(l, nav(NavigateTarget.Folder, "f1"), 0))
        assertEquals("f1", memory.folderFor(l)?.id)
        memory.navigate(l, nav(NavigateTarget.Folder, "f2"), 0)
        assertEquals("f2", memory.folderFor(l)?.id)
        memory.navigate(l, nav(NavigateTarget.Back), 0)
        assertEquals("f1", memory.folderFor(l)?.id)
        memory.navigate(l, nav(NavigateTarget.Back), 0)
        assertNull(memory.folderFor(l))
        // Unknown folder: nothing happens.
        memory.navigate(l, nav(NavigateTarget.Folder, "nope"), 0)
        assertNull(memory.folderFor(l))
    }

    @Test
    fun aFolderThatDisappearsOrAnotherProfileShowsThePagesAgain() {
        val memory = PageMemory()
        memory.navigate(layout("a", "p1"), nav(NavigateTarget.Folder, "f1"), 0)
        assertNull(memory.folderFor(layout("a", "p1", folders = listOf("f2"))))
        memory.navigate(layout("a", "p1"), nav(NavigateTarget.Folder, "f1"), 0)
        assertNull(memory.folderFor(layout("b", "p1")))
    }

    @Test
    fun pageNavigationClosesFoldersAndStaysInBounds() {
        val l = layout("a", "p1", "p2", "p3")
        val memory = PageMemory()
        memory.navigate(l, nav(NavigateTarget.Folder, "f1"), 0)
        assertEquals(2, memory.navigate(l, nav(NavigateTarget.Page, "p3"), 0))
        assertNull(memory.folderFor(l))
        assertEquals(2, memory.navigate(l, nav(NavigateTarget.Next), 2))
        assertEquals(1, memory.navigate(l, nav(NavigateTarget.Previous), 2))
        assertEquals(0, memory.navigate(l, nav(NavigateTarget.Previous), 0))
        assertNull(memory.navigate(l, nav(NavigateTarget.Page, "nope"), 1))
    }
}
