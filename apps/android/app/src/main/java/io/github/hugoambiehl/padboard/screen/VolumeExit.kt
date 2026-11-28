package io.github.hugoambiehl.padboard.screen

/**
 * Leaving the second screen with a long press on a volume key (#20).
 *
 * While the second screen shows, a volume key pressed is consumed whole,
 * down, repeats and up, until it is released, even after the second screen
 * has closed: the system's volume panel would otherwise show over the
 * picture, or over the grid with the repeats that keep coming (checked on
 * the SM-T580). The first repeat (`repeatCount` 1, Android's long press,
 * about 250–500 ms) leaves. Main thread only.
 */
class VolumeExit {
    /** The volume key held since the second screen showed, until released. */
    private var held: Int? = null

    enum class Outcome {
        /** Not ours: Android handles it. */
        Pass,

        /** Swallowed (no volume panel). */
        Consume,

        /** Swallowed, and the second screen must close. */
        Exit
    }

    fun onKey(keyCode: Int, down: Boolean, repeatCount: Int, streaming: Boolean): Outcome {
        if (keyCode != KEYCODE_VOLUME_UP && keyCode != KEYCODE_VOLUME_DOWN) return Outcome.Pass
        if (!down) {
            if (held != keyCode) return Outcome.Pass
            held = null
            return Outcome.Consume
        }
        if (repeatCount == 0 && streaming) held = keyCode
        if (held != keyCode) return Outcome.Pass
        return if (repeatCount == 1 && streaming) Outcome.Exit else Outcome.Consume
    }

    companion object {
        /** `KeyEvent.KEYCODE_VOLUME_UP` / `_DOWN` (copied: tested on the JVM). */
        const val KEYCODE_VOLUME_UP = 24
        const val KEYCODE_VOLUME_DOWN = 25
    }
}
