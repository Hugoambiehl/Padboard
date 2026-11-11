package io.github.hugoambiehl.padboard.ui.grid

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.hugoambiehl.padboard.protocol.Folder
import io.github.hugoambiehl.padboard.protocol.Layout
import io.github.hugoambiehl.padboard.protocol.NavigatePayload
import io.github.hugoambiehl.padboard.protocol.NavigateTarget

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
 *
 * Folders open on top of the pages (`navigate`), innermost last; one that
 * disappears from the layout is simply skipped, and another profile closes
 * them all.
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

    /** Open folders, innermost last (Compose state: the grid shows the innermost). */
    private var openFolders by mutableStateOf(OpenFolders("", emptyList()))

    private data class OpenFolders(val profileId: String, val ids: List<String>)

    /** The folder to show instead of the pages, if any. */
    fun folderFor(layout: Layout): Folder? {
        val open = openFolders
        if (open.profileId != layout.profile.id) return null
        return open.ids.asReversed().firstNotNullOfOrNull { id -> layout.folders.firstOrNull { it.id == id } }
    }

    /**
     * Applies a `navigate` request. Returns the page index to scroll to, or
     * `null` when the pages do not move (folder opened or closed, unknown id).
     */
    fun navigate(layout: Layout, request: NavigatePayload, currentIndex: Int): Int? {
        val stillOpen =
            if (openFolders.profileId == layout.profile.id) {
                openFolders.ids.filter { id -> layout.folders.any { it.id == id } }
            } else {
                emptyList()
            }
        return when (request.to) {
            NavigateTarget.Folder -> {
                val id = request.id?.takeIf { id -> layout.folders.any { it.id == id } } ?: return null
                openFolders = OpenFolders(layout.profile.id, stillOpen - id + id)
                null
            }
            NavigateTarget.Back -> {
                openFolders = OpenFolders(layout.profile.id, stillOpen.dropLast(1))
                null
            }
            NavigateTarget.Page -> {
                val index = layout.pages.indexOfFirst { it.id == request.id }
                if (index < 0) return null
                closeFolders()
                index
            }
            NavigateTarget.Next, NavigateTarget.Previous -> {
                if (layout.pages.isEmpty()) return null
                closeFolders()
                val step = if (request.to == NavigateTarget.Next) 1 else -1
                (currentIndex + step).coerceIn(0, layout.pages.lastIndex)
            }
        }
    }

    fun closeFolders() {
        openFolders = OpenFolders("", emptyList())
    }
}
