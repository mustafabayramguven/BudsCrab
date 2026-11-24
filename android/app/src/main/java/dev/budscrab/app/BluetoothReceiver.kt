package dev.budscrab.app

import android.bluetooth.BluetoothDevice
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.content.IntentCompat

/** Starts the service when the selected buds connect, stops it when they go away. */
class BluetoothReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val device = IntentCompat.getParcelableExtra(intent, BluetoothDevice.EXTRA_DEVICE, BluetoothDevice::class.java)
            ?: return
        val prefs = Buds.prefs
        if (device.address != prefs.deviceAddress) return
        when (intent.action) {
            BluetoothDevice.ACTION_ACL_CONNECTED -> if (prefs.autoConnect) {
                try {
                    Buds.startService(context)
                } catch (e: Exception) {
                    // Background start can be refused on some ROMs; the user can open the app.
                    Log.w("BudsCrab", "auto start refused: $e")
                }
            }
            BluetoothDevice.ACTION_ACL_DISCONNECTED ->
                context.stopService(Intent(context, BudsService::class.java))
        }
    }
}
