package io.github.hugoambiehl.padboard.kiosk

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ExitGestureTest {
    // The SM-T580 in landscape, in pixels (1.5 px per dp): a 40 dp corner is 60 px.
    private fun inCorner(x: Float, y: Float) = inKioskExitCorner(x, y, 1920f, 1200f, 60f)

    @Test
    fun onlyTheBottomLeftCornerCounts() {
        assertTrue(inCorner(0f, 1200f))
        assertTrue(inCorner(59f, 1141f))
        assertFalse("just above", inCorner(10f, 1139f))
        assertFalse("just right", inCorner(61f, 1190f))
        // The other corners, where the connection pill and the clock are.
        assertFalse(inCorner(1910f, 10f))
        assertFalse(inCorner(10f, 10f))
        assertFalse(inCorner(1910f, 1190f))
    }
}
