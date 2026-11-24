package dev.budscrab.app

import android.Manifest
import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothSocket
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.media.AudioManager
import android.media.AudioPlaybackConfiguration
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.core.os.HandlerCompat
import dev.budscrab.app.ui.MainActivity
import dev.budscrab.app.ui.label
import dev.budscrab.core.NoiseControl
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.IOException
import java.util.TimeZone
import java.util.UUID

/**
 * Foreground service that owns the RFCOMM socket to the Buds Core.
 * Everything protocol related is delegated to the Rust core via [Buds].
 */
@SuppressLint("MissingPermission")
class BudsService : Service() {
    companion object {
        const val ACTION_START = "dev.budscrab.START"
        const val ACTION_STOP = "dev.budscrab.STOP"
        const val ACTION_NOISE = "dev.budscrab.NOISE"
        const val EXTRA_MODE = "mode"
        private const val NOTIF_ID = 1
        private const val TAG = "BudsCrab"
        private val PAUSE_TOKEN = Any()
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var connectJob: Job? = null
    @Volatile private var socket: BluetoothSocket? = null
    private val main = Handler(Looper.getMainLooper())

    private var playbackCallback: AudioManager.AudioPlaybackCallback? = null
    private var wasPlaying = false
    private var netCallback: ConnectivityManager.NetworkCallback? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                shutdown()
                return START_NOT_STICKY
            }
            ACTION_NOISE -> {
                val mode = intent.getStringExtra(EXTRA_MODE)?.let { runCatching { NoiseControl.valueOf(it) }.getOrNull() }
                if (mode != null) Buds.cmd { setNoiseControl(mode, it) }
                return START_STICKY
            }
        }
        if (!hasBtPermission()) {
            Buds.notice("Bluetooth izni gerekli")
            stopSelf()
            return START_NOT_STICKY
        }
        try {
            ServiceCompat.startForeground(
                this, NOTIF_ID, buildNotification(),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE else 0
            )
        } catch (e: Exception) {
            Log.e(TAG, "startForeground failed", e)
            stopSelf()
            return START_NOT_STICKY
        }
        if (connectJob?.isActive != true) {
            connectJob = scope.launch { connectLoop() }
            startObservers()
        }
        return START_STICKY
    }

    private fun hasBtPermission(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.S ||
            ContextCompat.checkSelfPermission(this, Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED

    // ---- connection ----------------------------------------------------------

    private suspend fun connectLoop() {
        val adapter = getSystemService(BluetoothManager::class.java)?.adapter
        if (adapter == null) {
            Buds.notice("Bluetooth yok")
            return
        }
        var backoff = 3_000L
        while (currentCoroutineContext().isActive) {
            val address = Buds.prefs.deviceAddress ?: break
            val name = Buds.prefs.deviceName ?: address
            if (!adapter.isEnabled) {
                Buds.setConn(ConnState.Waiting("Bluetooth kapalı"))
                delay(5_000)
                continue
            }
            Buds.setConn(ConnState.Connecting(name))
            var reason = "bağlantı kapandı"
            var sock: BluetoothSocket? = null
            try {
                val device = adapter.getRemoteDevice(address)
                sock = device.createRfcommSocketToServiceRecord(UUID.fromString(Buds.session.sppUuid()))
                // cancelDiscovery() needs BLUETOOTH_SCAN on Android 12+; we never
                // start discovery, so only cancel when that permission exists.
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S ||
                    ContextCompat.checkSelfPermission(this@BudsService, Manifest.permission.BLUETOOTH_SCAN) == PackageManager.PERMISSION_GRANTED
                ) {
                    runCatching { adapter.cancelDiscovery() }
                }
                sock.connect()
                socket = sock
                Buds.output = sock.outputStream
                Buds.setConn(ConnState.Connected(name))
                backoff = 3_000L
                val tz = TimeZone.getDefault().getOffset(System.currentTimeMillis())
                Buds.dispatch(Buds.session.onConnected(Buds.now(), Build.VERSION.SDK_INT, tz))

                val ticker = scope.launch {
                    while (isActive) {
                        delay(200)
                        if (Buds.session.hasPendingHold()) Buds.dispatch(Buds.session.tick(Buds.now()))
                    }
                }
                val input = sock.inputStream
                val buf = ByteArray(4096)
                try {
                    while (currentCoroutineContext().isActive) {
                        val n = input.read(buf)
                        if (n < 0) break
                        if (n > 0) Buds.dispatch(Buds.session.feed(buf.copyOf(n), Buds.now()))
                    }
                } finally {
                    ticker.cancel()
                }
            } catch (e: IOException) {
                reason = e.message ?: e.javaClass.simpleName
                Log.i(TAG, "RFCOMM: $reason")
            } catch (e: SecurityException) {
                reason = "izin hatası: ${e.message}"
                Log.e(TAG, "SecurityException", e)
                Buds.notice("Bluetooth izin hatası: ${e.message}")
                break
            } finally {
                Buds.output = null
                socket = null
                runCatching { sock?.close() }
                Buds.dispatch(Buds.session.onDisconnected(Buds.now(), reason))
            }
            if (!currentCoroutineContext().isActive) break
            Buds.setConn(ConnState.Waiting("Yeniden deneniyor… ($reason)"))
            delay(backoff)
            backoff = (backoff * 2).coerceAtMost(30_000L)
        }
        Buds.setConn(ConnState.Idle)
    }

    // ---- observers -----------------------------------------------------------

    private fun startObservers() {
        // Keep the notification in sync with the buds.
        scope.launch {
            Buds.state.combine(Buds.conn) { s, c -> s to c }.collectLatest {
                delay(300)
                getSystemService(NotificationManager::class.java).notify(NOTIF_ID, buildNotification())
            }
        }
        startPauseDetective()
        startWifiWatcher()
    }

    /** Notices when music stops and asks the core to guess why. */
    private fun startPauseDetective() {
        if (!Buds.prefs.pauseDetective) return
        val am = getSystemService(AudioManager::class.java)
        wasPlaying = am.isMusicActive
        val cb = object : AudioManager.AudioPlaybackCallback() {
            override fun onPlaybackConfigChanged(configs: MutableList<AudioPlaybackConfiguration>?) {
                main.removeCallbacksAndMessages(PAUSE_TOKEN)
                HandlerCompat.postDelayed(main, {
                    val playing = am.isMusicActive
                    if (wasPlaying && !playing) {
                        Buds.setLastPause(Buds.session.onMediaPaused(Buds.now()))
                    }
                    wasPlaying = playing
                }, PAUSE_TOKEN, 400)
            }
        }
        am.registerAudioPlaybackCallback(cb, main)
        playbackCallback = cb
    }

    private fun startWifiWatcher() {
        val cfg = Buds.session.config()
        if (cfg.wifiProfiles.isEmpty()) return
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED) return
        val cm = getSystemService(ConnectivityManager::class.java)
        val req = NetworkRequest.Builder().addTransportType(NetworkCapabilities.TRANSPORT_WIFI).build()
        val cb = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            object : ConnectivityManager.NetworkCallback(FLAG_INCLUDE_LOCATION_INFO) {
                override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                    val ssid = (caps.transportInfo as? WifiInfo)?.ssid
                    Buds.dispatch(Buds.session.onWifiChanged(ssid, Buds.now()))
                }

                override fun onLost(network: Network) {
                    Buds.dispatch(Buds.session.onWifiChanged(null, Buds.now()))
                }
            }
        } else {
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    @Suppress("DEPRECATION")
                    val ssid = getSystemService(WifiManager::class.java)?.connectionInfo?.ssid
                    Buds.dispatch(Buds.session.onWifiChanged(ssid, Buds.now()))
                }

                override fun onLost(network: Network) {
                    Buds.dispatch(Buds.session.onWifiChanged(null, Buds.now()))
                }
            }
        }
        cm.registerNetworkCallback(req, cb)
        netCallback = cb
    }

    // ---- notification --------------------------------------------------------

    private fun pi(action: String, mode: NoiseControl? = null, req: Int = 0): PendingIntent {
        val i = Intent(this, BudsService::class.java).setAction(action)
        if (mode != null) i.putExtra(EXTRA_MODE, mode.name)
        return PendingIntent.getService(this, req, i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    private fun buildNotification(): Notification {
        val s = Buds.state.value
        val c = Buds.conn.value
        val title = when (c) {
            is ConnState.Connected -> c.name
            is ConnState.Connecting -> "Bağlanıyor: ${c.name}"
            is ConnState.Waiting -> "Bekleniyor"
            ConnState.Idle -> "BudsCrab"
        }
        val text = if (s != null && s.ready) {
            val l = s.batteryLeft?.let { "Sol %$it${if (s.chargingLeft) "⚡" else ""}" } ?: "Sol -"
            val r = s.batteryRight?.let { "Sağ %$it${if (s.chargingRight) "⚡" else ""}" } ?: "Sağ -"
            val k = s.batteryCase?.takeIf { it > 0 }?.let { " · Kutu %$it" } ?: ""
            "$l · $r$k · ${s.noiseControl?.label() ?: ""}"
        } else if (c is ConnState.Waiting) c.reason else "Kulaklık bekleniyor"

        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE
        )
        val b = NotificationCompat.Builder(this, BudsCrabApp.CH_SERVICE)
            .setSmallIcon(R.drawable.ic_tile)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
        if (s?.ready == true) {
            b.addAction(0, "ANC", pi(ACTION_NOISE, NoiseControl.ANC, 1))
            b.addAction(0, "Ortam", pi(ACTION_NOISE, NoiseControl.AMBIENT, 2))
            b.addAction(0, "Kapalı", pi(ACTION_NOISE, NoiseControl.OFF, 3))
        } else {
            b.addAction(0, "Durdur", pi(ACTION_STOP, req = 4))
        }
        return b.build()
    }

    // ---- teardown ------------------------------------------------------------

    private fun shutdown() {
        connectJob?.cancel()
        runCatching { socket?.close() }
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        playbackCallback?.let { getSystemService(AudioManager::class.java).unregisterAudioPlaybackCallback(it) }
        netCallback?.let { runCatching { getSystemService(ConnectivityManager::class.java).unregisterNetworkCallback(it) } }
        runCatching { socket?.close() }
        scope.cancel()
        Buds.output = null
        Buds.setConn(ConnState.Idle)
        super.onDestroy()
    }
}
