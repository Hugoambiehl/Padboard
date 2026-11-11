package io.github.hugoambiehl.padboard.ui.grid

import io.github.hugoambiehl.padboard.protocol.Layout

/** The page in front of the user, by id, and where to fall back if it disappears. */
data class ShownPage(val profileId: String, val pageId: String, val fallbackIndex: Int)

/**
 * Which page the user is on. Owned by the ViewModel, so it survives screen
 * changes, reconnections and USB ↔ Wi-Fi switches; not app restarts (the
 * grid then starts on the first page). Used from the UI thread only.
 *
 * Pages are followed by id, never by position:
 * - the page shown still exists in the same profile: that page, wherever it moved;
 * - another profile, or nothing shown yet: its first page;
 * - the page shown was deleted while connected (seen live): the page now at
 *   its position (the last one if it was the last);
 * - the page disappeared while disconnected: the first page.
 */
class PageMemory {
    var shown: ShownPage? = null
        private set

    /** Index of the page to show in `layout`. */
    fun pageFor(layout: Layout): Int {
        val shown = shown
        if (layout.pages.isEmpty() || shown == null || shown.profileId != layout.profile.id) return 0
        val kept = layout.pages.indexOfFirst { it.id == shown.pageId }
        if (kept >= 0) return kept
        return shown.fallbackIndex.coerceIn(0, layout.pages.lastIndex)
    }

    /** The user now sees page `index` of `layout`. */
    fun onShown(layout: Layout, index: Int) {
        val page = layout.pages.getOrNull(index) ?: return
        shown = ShownPage(layout.profile.id, page.id, index)
    }

    /**
     * The link dropped: edits made meanwhile were not seen, so a page that
     * disappeared sends the user back to the first page.
     */
    fun onConnectionLost() {
        shown = shown?.copy(fallbackIndex = 0)
    }
}
