package io.github.hugoambiehl.padboard.screen

import io.github.hugoambiehl.padboard.screen.VolumeExit.Companion.KEYCODE_VOLUME_DOWN
import io.github.hugoambiehl.padboard.screen.VolumeExit.Companion.KEYCODE_VOLUME_UP
import io.github.hugoambiehl.padboard.screen.VolumeExit.Outcome
import org.junit.Assert.assertEquals
import org.junit.Test

class VolumeExitTest {
    private val exit = VolumeExit()

    @Test
    fun aLongPressLeavesAndTheKeyIsSwallowedUntilReleased() {
        assertEquals(Outcome.Consume, exit.onKey(KEYCODE_VOLUME_DOWN, down = true, repeatCount = 0, streaming = true))
        assertEquals(Outcome.Exit, exit.onKey(KEYCODE_VOLUME_DOWN, down = true, repeatCount = 1, streaming = true))
        // The second screen closed: the repeats would show the volume panel.
        assertEquals(Outcome.Consume, exit.onKey(KEYCODE_VOLUME_DOWN, down = true, repeatCount = 2, streaming = false))
        assertEquals(Outcome.Consume, exit.onKey(KEYCODE_VOLUME_DOWN, down = false, repeatCount = 0, streaming = false))
        // Released: the next press is Android's again.
        assertEquals(Outcome.Pass, exit.onKey(KEYCODE_VOLUME_DOWN, down = true, repeatCount = 0, streaming = false))
    }

    @Test
    fun aShortPressIsSwallowedWithoutLeaving() {
        assertEquals(Outcome.Consume, exit.onKey(KEYCODE_VOLUME_UP, down = true, repeatCount = 0, streaming = true))
        assertEquals(Outcome.Consume, exit.onKey(KEYCODE_VOLUME_UP, down = false, repeatCount = 0, streaming = true))
    }

    @Test
    fun otherKeysAndTheGridAreLeftToAndroid() {
        assertEquals(Outcome.Pass, exit.onKey(KEYCODE_VOLUME_UP, down = true, repeatCount = 0, streaming = false))
        assertEquals(Outcome.Pass, exit.onKey(KEYCODE_VOLUME_UP, down = true, repeatCount = 1, streaming = false))
        assertEquals(Outcome.Pass, exit.onKey(4, down = true, repeatCount = 0, streaming = true))
    }

    @Test
    fun aKeyHeldBeforeTheSecondScreenIsNotTakenOver() {
        // Held on the grid, the second screen opens meanwhile: still Android's.
        assertEquals(Outcome.Pass, exit.onKey(KEYCODE_VOLUME_UP, down = true, repeatCount = 0, streaming = false))
        assertEquals(Outcome.Pass, exit.onKey(KEYCODE_VOLUME_UP, down = true, repeatCount = 1, streaming = true))
        assertEquals(Outcome.Pass, exit.onKey(KEYCODE_VOLUME_UP, down = false, repeatCount = 0, streaming = true))
    }
}
