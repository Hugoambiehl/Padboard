package io.github.hugoambiehl.padboard.protocol

/**
 * The layout with `view` in place of the button of the same id (`button:update`),
 * in a page or a folder. Unknown ids leave the layout unchanged (same instance),
 * so nothing is recomposed.
 */
fun Layout.withButton(view: ButtonView): Layout {
    fun List<ButtonView>.replaced(): List<ButtonView>? {
        val index = indexOfFirst { it.id == view.id }
        if (index < 0 || this[index] == view) return null
        return toMutableList().also { it[index] = view }
    }
    pages.forEachIndexed { i, page ->
        val buttons = page.buttons.replaced() ?: return@forEachIndexed
        return copy(pages = pages.toMutableList().also { it[i] = page.copy(buttons = buttons) })
    }
    folders.forEachIndexed { i, folder ->
        val buttons = folder.buttons.replaced() ?: return@forEachIndexed
        return copy(folders = folders.toMutableList().also { it[i] = folder.copy(buttons = buttons) })
    }
    return this
}
