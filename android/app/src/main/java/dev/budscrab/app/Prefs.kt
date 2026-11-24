package dev.budscrab.app

import android.content.Context

/** Tiny SharedPreferences wrapper for things the Rust core does not own. */
class Prefs(context: Context) {
    private val sp = context.getSharedPreferences("budscrab", Context.MODE_PRIVATE)

    var deviceAddress: String?
        get() = sp.getString("device_address", null)
        set(v) = sp.edit().putString("device_address", v).apply()

    var deviceName: String?
        get() = sp.getString("device_name", null)
        set(v) = sp.edit().putString("device_name", v).apply()

    /** Start the service automatically when the buds connect. */
    var autoConnect: Boolean
        get() = sp.getBoolean("auto_connect", true)
        set(v) = sp.edit().putBoolean("auto_connect", v).apply()

    /** Track why music pauses (pause detective). */
    var pauseDetective: Boolean
        get() = sp.getBoolean("pause_detective", true)
        set(v) = sp.edit().putBoolean("pause_detective", v).apply()
}
