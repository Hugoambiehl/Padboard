package io.github.hugoambiehl.padboard.ui.grid

import io.github.hugoambiehl.padboard.protocol.Layout

/**
 * The page to show after a layout update. Pages are followed by id, never by
 * position: an edit of another page, a page added or moved keeps the page in
 * front of the user.
 *
 * - Another profile (or the first layout): its first page.
 * - The page shown still exists: that page, wherever it moved.
 * - The page shown was deleted: the page now at its position (the last one
 *   if it was the last).
 */
internal fun pageAfterUpdate(previous: Layout?, previousIndex: Int, next: Layout): Int {
    if (next.pages.isEmpty()) return 0
    if (previous == null || previous.profile.id != next.profile.id) return 0
    val shownId = previous.pages.getOrNull(previousIndex)?.id
    val kept = next.pages.indexOfFirst { it.id == shownId }
    if (kept >= 0) return kept
    return previousIndex.coerceIn(0, next.pages.lastIndex)
}
