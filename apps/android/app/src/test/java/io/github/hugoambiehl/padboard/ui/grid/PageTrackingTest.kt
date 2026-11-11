package io.github.hugoambiehl.padboard.ui.grid

import io.github.hugoambiehl.padboard.protocol.GridSize
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.Page
import io.github.hugoambiehl.padboard.protocol.ProfileInfo
import org.junit.Assert.assertEquals
import org.junit.Test

class PageTrackingTest {
    private fun layout(profile: String, vararg pages: String) = Layout(
        profile = ProfileInfo(profile, profile),
        grid = GridSize(5u, 3u),
        pages = pages.map { Page(it, emptyList()) }
    )

    @Test
    fun theFirstLayoutAndAnotherProfileStartOnTheFirstPage() {
        assertEquals(0, pageAfterUpdate(null, 3, layout("a", "p1", "p2")))
        assertEquals(0, pageAfterUpdate(layout("a", "p1", "p2"), 1, layout("b", "p1", "p2")))
    }

    @Test
    fun theShownPageIsFollowedByIdWhenPagesMove() {
        val before = layout("a", "p1", "p2", "p3")
        assertEquals(1, pageAfterUpdate(before, 1, layout("a", "p1", "p2", "p3")))
        // A page added before it, then the pages reordered.
        assertEquals(2, pageAfterUpdate(before, 1, layout("a", "p0", "p1", "p2", "p3")))
        assertEquals(0, pageAfterUpdate(before, 1, layout("a", "p2", "p1", "p3")))
    }

    @Test
    fun aDeletedPageGivesWayToThePageAtItsPosition() {
        val before = layout("a", "p1", "p2", "p3")
        assertEquals(1, pageAfterUpdate(before, 1, layout("a", "p1", "p3")))
        // It was the last one: the new last page.
        assertEquals(1, pageAfterUpdate(before, 2, layout("a", "p1", "p2")))
        assertEquals(0, pageAfterUpdate(before, 2, layout("a", "p1")))
    }
}
