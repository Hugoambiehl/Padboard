package io.github.hugoambiehl.padboard

import android.app.ActivityManager
import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
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
import io.github.hugoambiehl.padboard.connection.LocalNetwork
import io.github.hugoambiehl.padboard.kiosk.Kiosk
import io.github.hugoambiehl.padboard.protocol.ScreenStopReason
import io.github.hugoambiehl.padboard.screen.VolumeExit
import io.github.hugoambiehl.padboard.ui.PadboardApp
import io.github.hugoambiehl.padboard.ui.PadboardViewModel
import io.github.hugoambiehl.padboard.ui.theme.PadboardTheme
import java.lang.ref.WeakReference
import java.util.logging.Logger
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val viewModel: PadboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        instances += WeakReference(this)
        if (isHome) Kiosk.homeScreens++
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

    private val endLockTask: () -> Unit = { runCatching { stopLockTask() } }

    override fun onResume() {
        super.onResume()
        closeOtherInstances()
        if (isFinishing) return
        Kiosk.stopLockTask = endLockTask
        // Device owner set or removed with adb while the app was away.
        Kiosk.refresh(this)
        // Local network allowed or revoked in the settings meanwhile (Android 17).
        LocalNetwork.refresh(this)
    }

    override fun onPause() {
        if (Kiosk.stopLockTask === endLockTask) Kiosk.stopLockTask = null
        super.onPause()
    }

    override fun onDestroy() {
        instances.removeAll { it.get().let { activity -> activity == null || activity === this } }
        if (isHome) {
            Kiosk.homeScreens--
            Kiosk.disableHome(this)
        }
        super.onDestroy()
    }

    /** Started as the home entry (kiosk mode), not by the launcher or the PC. */
    private val isHome get() = componentName.className == Kiosk.HOME_ALIAS

    /** This instance already handed kiosk mode to the home entry (once). */
    private var homeRequested = false

    /**
     * One screen at a time. In kiosk mode on Android 12L, the activity the
     * PC started (a standard task) and the home entry the system starts at
     * the first wake-up (a home task) lived side by side. The connection is
     * shared anyway ([io.github.hugoambiehl.padboard.connection.TabletLink]),
     * but a hidden second screen costs memory.
     *
     * In kiosk mode the home entry stays (closed, the system starts it
     * again at once) and the other screens leave; they never lock their
     * task (see [followKiosk]): Android never lets the root of a lock task
     * finish. Otherwise (kiosk mode just left: [Kiosk.exit] brings the app
     * back as itself) the newer screen stays.
     */
    private fun closeOtherInstances() {
        val others = instances.mapNotNull { it.get() }.filter { it !== this && !it.isFinishing }
        if (others.isEmpty()) return
        if (!Kiosk.active.value || isHome) {
            log.info("${others.size} other instance(s) of the app's screen: closed")
            others.forEach { it.finish() }
        } else if (others.any { it.isHome }) {
            log.info("kiosk mode runs in the home entry: this screen closes")
            finish()
        }
    }

    /** Kiosk mode: locked in front, screen kept on (the screensaver dims it). */
    private fun followKiosk(on: Boolean) {
        if (isFinishing) return
        if (on && !isHome && !homeRequested) {
            // Kiosk mode runs in the home entry, the screen the system starts
            // at boot and at wake-up: go there before locking anything. On
            // Android 12L it opens in a task of its own and closes this one;
            // Android 7 hands the intent to this screen, which locks below
            // when it comes back.
            homeRequested = true
            log.info("kiosk mode: handing over to the home entry")
            startActivity(Kiosk.homeIntent(this))
            return
        }
        if (!on && isHome) {
            // Kiosk mode left: Android closes the home entry's screens once it
            // is disabled (in onDestroy). The app reopens as itself, in a new
            // task: finished first, this screen no longer takes the intent.
            log.info("kiosk mode left: the app reopens as itself")
            finish()
            startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            return
        }
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

    private val volumeExit = VolumeExit()

    /** Volume keys during the second screen: see [VolumeExit]. */
    private fun volumeKey(keyCode: Int, event: KeyEvent, down: Boolean): Boolean {
        val offer = viewModel.screen.value
        return when (volumeExit.onKey(keyCode, down, event.repeatCount, streaming = offer != null)) {
            VolumeExit.Outcome.Pass -> false
            VolumeExit.Outcome.Consume -> true
            VolumeExit.Outcome.Exit -> {
                offer?.let { viewModel.stopScreen(it, ScreenStopReason.User) }
                true
            }
        }
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean =
        volumeKey(keyCode, event, down = true) || super.onKeyDown(keyCode, event)

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean =
        volumeKey(keyCode, event, down = false) || super.onKeyUp(keyCode, event)

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

    private companion object {
        val log: Logger = Logger.getLogger("MainActivity")

        /** The live instances (main thread only). */
        val instances = mutableListOf<WeakReference<MainActivity>>()
    }
}
