package io.github.hugoambiehl.padboard.connection

import io.github.hugoambiehl.padboard.protocol.KioskLeft

/**
 * Exits from kiosk mode the PC has not been told about yet (kept by the app,
 * see `kiosk.KioskExits`): sent once, in the next `tablet:status`.
 */
interface KioskExitLog {
    /** The exit to report, with its age; null when there is none. */
    fun pending(): KioskLeft?

    /** It was sent to the PC. */
    fun reported()

    /** Nothing to report (tests). */
    object None : KioskExitLog {
        override fun pending(): KioskLeft? = null

        override fun reported() = Unit
    }
}
