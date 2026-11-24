package dev.budscrab.app.ui

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.GraphicEq
import androidx.compose.material.icons.filled.Headphones
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.TouchApp
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import dev.budscrab.app.Buds

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent { BudsCrabTheme { Root() } }
    }
}

private val requiredPermissions: Array<String>
    get() = buildList {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) add(Manifest.permission.BLUETOOTH_CONNECT)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) add(Manifest.permission.POST_NOTIFICATIONS)
    }.toTypedArray()

private data class Tab(val title: String, val icon: ImageVector)

private val tabs = listOf(
    Tab("Pano", Icons.Filled.Headphones),
    Tab("Dokunma", Icons.Filled.TouchApp),
    Tab("Ses", Icons.Filled.GraphicEq),
    Tab("Bul", Icons.Filled.Search),
    Tab("Daha", Icons.Filled.MoreHoriz),
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Root() {
    val ctx = LocalContext.current
    fun granted() = requiredPermissions.all {
        ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED
    }
    var hasPerms by remember { mutableStateOf(granted()) }
    var pickDevice by remember { mutableStateOf(Buds.prefs.deviceAddress == null) }
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val snackbar = remember { SnackbarHostState() }
    val permLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        hasPerms = granted()
    }

    LaunchedEffect(Unit) {
        Buds.notices.collect { snackbar.showSnackbar(it) }
    }
    LaunchedEffect(hasPerms, pickDevice) {
        // Opening the app connects automatically once a device is chosen.
        if (hasPerms && !pickDevice && !Buds.isConnected) Buds.startService(ctx)
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text(if (pickDevice || !hasPerms) "BudsCrab" else tabs[tab].title) }) },
        snackbarHost = { SnackbarHost(snackbar) },
        bottomBar = {
            if (hasPerms && !pickDevice) {
                NavigationBar {
                    tabs.forEachIndexed { i, t ->
                        NavigationBarItem(
                            selected = tab == i, onClick = { tab = i },
                            icon = { Icon(t.icon, contentDescription = t.title) },
                            label = { Text(t.title) }
                        )
                    }
                }
            }
        }
    ) { pad ->
        Column(Modifier.padding(pad).fillMaxSize()) {
            when {
                !hasPerms -> PermissionScreen { permLauncher.launch(requiredPermissions) }
                pickDevice -> DevicePicker { pickDevice = false }
                else -> when (tab) {
                    0 -> HomeScreen()
                    1 -> TouchScreen()
                    2 -> SoundScreen()
                    3 -> FindScreen()
                    else -> MoreScreen(onChangeDevice = {
                        Buds.stopService(ctx)
                        pickDevice = true
                    })
                }
            }
        }
    }
}

@Composable
private fun PermissionScreen(request: () -> Unit) {
    Column(Modifier.padding(16.dp)) {
        Section("İzinler") {
            Text("Kulaklıkla konuşmak için Bluetooth bağlantı iznine, pil bildirimi için bildirim iznine ihtiyacım var.")
            Hint("BudsCrab internete hiç bağlanmaz, Google Play gerektirmez.")
            Button(onClick = request) { Text("İzin ver") }
        }
    }
}

@SuppressLint("MissingPermission")
@Composable
private fun DevicePicker(onPicked: () -> Unit) {
    val ctx = LocalContext.current
    val devices = remember {
        val adapter = ctx.getSystemService(BluetoothManager::class.java)?.adapter
        val bonded: List<BluetoothDevice> = try {
            adapter?.bondedDevices?.toList().orEmpty()
        } catch (_: SecurityException) {
            emptyList()
        }
        // Buds first
        bonded.sortedByDescending { (it.name ?: "").contains("Buds", ignoreCase = true) }
    }
    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
        Section("Kulaklığını seç") {
            Hint("Listede yoksa önce Android Bluetooth ayarlarından Buds Core ile eşleş.")
            if (devices.isEmpty()) Text("Eşleşmiş cihaz yok")
            devices.forEach { d ->
                val name = d.name ?: d.address
                Column(
                    Modifier.fillMaxWidth().clickable {
                        Buds.prefs.deviceAddress = d.address
                        Buds.prefs.deviceName = name
                        onPicked()
                    }.padding(vertical = 10.dp)
                ) {
                    Text(name, style = MaterialTheme.typography.bodyLarge)
                    Hint(d.address + if (name.contains("Core", true)) "  ·  Buds Core" else "")
                }
            }
        }
    }
}
