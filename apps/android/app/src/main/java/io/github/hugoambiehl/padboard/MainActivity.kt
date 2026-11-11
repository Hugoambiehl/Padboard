package io.github.hugoambiehl.padboard

import android.app.ActivityManager
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import io.github.hugoambiehl.padboard.kiosk.Kiosk
import io.github.hugoambiehl.padboard.ui.PadboardApp
import io.github.hugoambiehl.padboard.ui.PadboardViewModel
import io.github.hugoambiehl.padboard.ui.theme.PadboardTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val viewModel: PadboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        hideSystemBars()
        setContent {
            PadboardTheme {
                PadboardApp(viewModel)
            }
        }
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) { Kiosk.active.collect(::followKiosk) }
        }
    }

    override fun onResume() {
        super.onResume()
        // Device owner set or removed with adb while the app was away.
        Kiosk.refresh(this)
    }

    /** Kiosk mode: locked in front, screen kept on (the screensaver dims it). */
    private fun followKiosk(on: Boolean) {
        val locked = getSystemService(ActivityManager::class.java).lockTaskModeState !=
            ActivityManager.LOCK_TASK_MODE_NONE
        if (on) {
            window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            if (!locked) startLockTask()
        } else {
            window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            if (locked) runCatching { stopLockTask() }
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideSystemBars()
    }

    /** Immersive mode: the tablet is dedicated to Padboard (kiosk mode also locks it, see Kiosk). */
    private fun hideSystemBars() {
        WindowCompat.getInsetsController(window, window.decorView).apply {
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            hide(WindowInsetsCompat.Type.systemBars())
        }
    }
}
