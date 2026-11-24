package dev.budscrab.app

import android.annotation.SuppressLint
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import dev.budscrab.core.AppAction
import dev.budscrab.core.BudSide
import dev.budscrab.core.Config
import dev.budscrab.core.CoreEvent
import dev.budscrab.core.DeviceState
import dev.budscrab.core.Session
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.OutputStream
import java.util.concurrent.Executors

sealed interface ConnState {
    data object Idle : ConnState
    data class Connecting(val name: String) : ConnState
    data class Connected(val name: String) : ConnState
    data class Waiting(val reason: String) : ConnState
}

/**
 * Process-wide glue between the Rust [Session], the foreground service that
 * owns the RFCOMM socket, and the Compose UI.
 */
@SuppressLint("StaticFieldLeak") // holds the application context only
object Buds {
    private const val TAG = "BudsCrab"

    lateinit var app: Context
        private set
    lateinit var session: Session
        private set
    lateinit var prefs: Prefs
        private set

    private val writer = Executors.newSingleThreadExecutor()

    @Volatile
    internal var output: OutputStream? = null

    private val _state = MutableStateFlow<DeviceState?>(null)
    val state = _state.asStateFlow()

    private val _conn = MutableStateFlow<ConnState>(ConnState.Idle)
    val conn = _conn.asStateFlow()

    private val _config = MutableStateFlow<Config?>(null)
    val config = _config.asStateFlow()

    private val _notices = MutableSharedFlow<String>(extraBufferCapacity = 16)
    val notices = _notices.asSharedFlow()

    private val _lastPause = MutableStateFlow<String?>(null)
    val lastPause = _lastPause.asStateFlow()

    fun init(context: Context) {
        app = context.applicationContext
        prefs = Prefs(app)
        session = Session(app.filesDir.absolutePath)
        _state.value = session.state()
        _config.value = session.config()
    }

    fun now(): Long = System.currentTimeMillis()

    internal fun setConn(c: ConnState) {
        _conn.value = c
    }

    internal fun setLastPause(reason: String) {
        _lastPause.value = reason
    }

    fun notice(msg: String) {
        _notices.tryEmit(msg)
    }

    val isConnected: Boolean get() = output != null && _state.value?.connected == true

    /** Run a core command and dispatch the resulting events. */
    fun cmd(block: Session.(Long) -> List<CoreEvent>) {
        if (output == null) {
            notice("Kulaklık bağlı değil")
            return
        }
        dispatch(session.block(now()))
    }

    fun perform(action: AppAction) = cmd { perform(action, it) }

    fun updateConfig(transform: (Config) -> Config) {
        val next = transform(session.config())
        session.setConfig(next)
        _config.value = next
    }

    fun dispatch(events: List<CoreEvent>) {
        for (e in events) {
            when (e) {
                is CoreEvent.Send -> write(e.bytes)
                is CoreEvent.StateChanged -> _state.value = e.state
                is CoreEvent.RunAction -> MediaActions.run(app, e.action)
                is CoreEvent.LowBattery -> lowBattery(e.side, e.level)
                is CoreEvent.Notice -> notice(e.message)
            }
        }
        _config.value = session.config()
    }

    private fun write(bytes: ByteArray) {
        val out = output ?: return
        writer.execute {
            try {
                out.write(bytes)
                out.flush()
            } catch (e: Exception) {
                Log.w(TAG, "write failed: $e")
            }
        }
    }

    private fun lowBattery(side: BudSide, level: Int) {
        val name = if (side == BudSide.LEFT) "Sol" else "Sağ"
        val n = NotificationCompat.Builder(app, BudsCrabApp.CH_ALERTS)
            .setSmallIcon(R.drawable.ic_tile)
            .setContentTitle("$name kulaklık pili düşük")
            .setContentText("%$level kaldı")
            .setAutoCancel(true)
            .build()
        try {
            app.getSystemService(NotificationManager::class.java).notify(100 + side.ordinal, n)
        } catch (_: SecurityException) {
        }
    }

    fun startService(context: Context) {
        if (prefs.deviceAddress == null) {
            notice("Önce kulaklığı seç")
            return
        }
        ContextCompat.startForegroundService(
            context, Intent(context, BudsService::class.java).setAction(BudsService.ACTION_START)
        )
    }

    fun stopService(context: Context) {
        context.startService(Intent(context, BudsService::class.java).setAction(BudsService.ACTION_STOP))
    }
}
