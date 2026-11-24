package dev.budscrab.app

import android.content.Context
import android.content.Intent
import android.media.AudioManager
import android.os.SystemClock
import android.util.Log
import android.view.KeyEvent
import dev.budscrab.core.AppAction

/** Phone-side actions triggered by custom touch-and-hold gestures. */
object MediaActions {
    private const val TAG = "BudsCrab"

    fun run(context: Context, action: AppAction) {
        val am = context.getSystemService(AudioManager::class.java)
        when (action) {
            AppAction.PLAY_PAUSE -> mediaKey(am, KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE)
            AppAction.NEXT_TRACK -> mediaKey(am, KeyEvent.KEYCODE_MEDIA_NEXT)
            AppAction.PREVIOUS_TRACK -> mediaKey(am, KeyEvent.KEYCODE_MEDIA_PREVIOUS)
            AppAction.VOLUME_UP ->
                am.adjustStreamVolume(AudioManager.STREAM_MUSIC, AudioManager.ADJUST_RAISE, AudioManager.FLAG_SHOW_UI)
            AppAction.VOLUME_DOWN ->
                am.adjustStreamVolume(AudioManager.STREAM_MUSIC, AudioManager.ADJUST_LOWER, AudioManager.FLAG_SHOW_UI)
            AppAction.VOICE_ASSISTANT -> {
                try {
                    context.startActivity(
                        Intent(Intent.ACTION_VOICE_COMMAND).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    )
                } catch (e: Exception) {
                    Log.w(TAG, "No voice assistant: $e")
                }
            }
            // Handled inside the Rust core (sends a command to the buds)
            AppAction.NOTHING, AppAction.CYCLE_NOISE_CONTROL, AppAction.TOGGLE_ANC, AppAction.TOGGLE_AMBIENT -> {}
        }
    }

    private fun mediaKey(am: AudioManager, code: Int) {
        val t = SystemClock.uptimeMillis()
        am.dispatchMediaKeyEvent(KeyEvent(t, t, KeyEvent.ACTION_DOWN, code, 0))
        am.dispatchMediaKeyEvent(KeyEvent(t, t, KeyEvent.ACTION_UP, code, 0))
    }
}
