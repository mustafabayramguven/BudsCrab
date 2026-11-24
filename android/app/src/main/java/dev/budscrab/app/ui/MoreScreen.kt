package dev.budscrab.app.ui

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.wifi.WifiManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.budscrab.app.Buds
import dev.budscrab.core.EqPreset
import dev.budscrab.core.NoiseControl
import dev.budscrab.core.WifiProfile
import dev.budscrab.core.decodeHex
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

private val timeFmt = SimpleDateFormat("dd.MM HH:mm:ss", Locale.getDefault())
private fun fmt(ms: Long) = timeFmt.format(Date(ms))

@Composable
fun MoreScreen(onChangeDevice: () -> Unit) {
    val st by Buds.state.collectAsStateWithLifecycle()
    val cfg by Buds.config.collectAsStateWithLifecycle()
    // Bumped to re-read logs / history from the core.
    var refresh by remember { mutableIntStateOf(0) }
    val s = st
    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
        if (s != null && s.ready) {
            Section("Gelişmiş") {
                s.seamlessConnection?.let { v ->
                    SwitchRow("Kesintisiz bağlantı", v, subtitle = "Samsung cihazlar arası otomatik geçiş") { on -> Buds.cmd { setSeamlessConnection(on, it) } }
                }
                s.sidetone?.let { v ->
                    SwitchRow("Aramada kendi sesini duy", v) { on -> Buds.cmd { setSidetone(on, it) } }
                }
                s.ancWithOneEarbud?.let { v ->
                    SwitchRow("Tek kulaklıkla ANC", v) { on -> Buds.cmd { setAncWithOneEarbud(on, it) } }
                }
                s.callPathControl?.let { v ->
                    SwitchRow("Kulaklığı takınca aramayı kulaklığa al", v) { on -> Buds.cmd { setCallPathControl(on, it) } }
                }
            }
        }

        cfg?.let { c ->
            Section("Uygulama") {
                var autoConnect by remember { mutableStateOf(Buds.prefs.autoConnect) }
                SwitchRow("Kulaklık bağlanınca otomatik başla", autoConnect) {
                    autoConnect = it
                    Buds.prefs.autoConnect = it
                }
                var detective by remember { mutableStateOf(Buds.prefs.pauseDetective) }
                SwitchRow("Duraklatma dedektifi", detective, subtitle = "Müzik durunca sebebini tahmin et (yeniden bağlanınca etkinleşir)") {
                    detective = it
                    Buds.prefs.pauseDetective = it
                }
                var thr by remember(c.lowBatteryThreshold) { mutableFloatStateOf(c.lowBatteryThreshold.toFloat()) }
                Text("Düşük pil uyarısı: %${thr.toInt()}" + if (thr.toInt() == 0) " (kapalı)" else "")
                Slider(
                    value = thr, onValueChange = { thr = it }, valueRange = 0f..50f, steps = 9,
                    onValueChangeFinished = { Buds.updateConfig { it.copy(lowBatteryThreshold = thr.toInt()) } }
                )
                OutlinedButton(onClick = onChangeDevice) { Text("Kulaklık değiştir (${Buds.prefs.deviceName ?: "seçilmedi"})") }
            }
            WifiProfiles(c.wifiProfiles)
        }

        BatteryHistory(refresh)
        Diagnostics(refresh) { refresh++ }
        Section("Hakkında") {
            Text("BudsCrab 0.1 — Galaxy Buds Core için açık kaynak yönetici")
            Hint("Rust çekirdek + Kotlin arayüz. Protokol bilgisi GalaxyBudsClient (Tim Schneeberger) projesinden. Lisans: GPL-3.0")
        }
    }
}

@Composable
private fun WifiProfiles(profiles: List<WifiProfile>) {
    val ctx = LocalContext.current
    var ssid by remember { mutableStateOf("") }
    var mode by remember { mutableStateOf<NoiseControl?>(NoiseControl.ANC) }
    var eq by remember { mutableStateOf<EqPreset?>(null) }
    val locLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    Section("Wi-Fi profilleri") {
        Hint("Belirli bir Wi-Fi'ye bağlanınca gürültü modunu / ekolayzırı otomatik değiştirir. Android, ağ adını okumak için konum izni ister.")
        profiles.forEach { p ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(p.ssid)
                    Hint(listOfNotNull(p.noiseControl?.label(), p.eq?.label()).joinToString(" · "))
                }
                IconButton(onClick = { Buds.updateConfig { c -> c.copy(wifiProfiles = c.wifiProfiles.filter { it.ssid != p.ssid }) } }) {
                    Icon(Icons.Filled.Delete, contentDescription = "Sil")
                }
            }
        }
        HorizontalDivider()
        OutlinedTextField(ssid, { ssid = it }, label = { Text("Ağ adı (SSID)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        TextButtonRow {
            OutlinedButton(onClick = {
                if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED) {
                    locLauncher.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                } else {
                    @Suppress("DEPRECATION")
                    val cur = ctx.getSystemService(WifiManager::class.java)?.connectionInfo?.ssid?.trim('"')
                    if (cur != null && cur != "<unknown ssid>") ssid = cur else Buds.notice("Wi-Fi adı okunamadı")
                }
            }) { Text("Şu anki ağ") }
        }
        Picker("Gürültü", mode, listOf(null) + NoiseControl.entries, { it?.label() ?: "Değiştirme" }) { mode = it }
        Picker("Ekolayzır", eq, listOf(null) + EqPreset.entries, { it?.label() ?: "Değiştirme" }) { eq = it }
        Button(enabled = ssid.isNotBlank(), onClick = {
            val p = WifiProfile(ssid.trim(), mode, eq)
            Buds.updateConfig { c -> c.copy(wifiProfiles = c.wifiProfiles.filter { it.ssid != p.ssid } + p) }
            ssid = ""
            Buds.notice("Profil kaydedildi. Etkinleşmesi için yeniden bağlan.")
        }) { Text("Profili ekle") }
    }
}

@Composable
private fun TextButtonRow(content: @Composable () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { content() }
}

@Composable
private fun BatteryHistory(refresh: Int) {
    var days by remember { mutableIntStateOf(1) }
    val now = remember(refresh, days) { System.currentTimeMillis() }
    val from = now - days * 24L * 3600_000L
    val samples = remember(refresh, days) { Buds.session.batteryHistory(from) }
    Section("Pil geçmişi") {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf(1 to "24 saat", 7 to "7 gün", 30 to "30 gün").forEach { (d, l) ->
                FilterChip(days == d, { days = d }, { Text(l) })
            }
        }
        if (samples.size < 2) {
            Hint("Henüz yeterli veri yok. Kulaklık bağlıyken her birkaç dakikada bir kayıt alınır.")
        } else {
            BatteryChart(samples, from, now)
        }
    }
}

@Composable
private fun Diagnostics(refresh: Int, onRefresh: () -> Unit) {
    val ctx = LocalContext.current
    var showPackets by remember { mutableStateOf(false) }
    var hex by remember { mutableStateOf("") }
    var decoded by remember { mutableStateOf("") }
    val diag = remember(refresh) { Buds.session.diagnostics() }
    val packets = remember(refresh, showPackets) { if (showPackets) Buds.session.packetLog() else emptyList() }
    Section("Tanılama") {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onRefresh) { Text("Yenile") }
            OutlinedButton(onClick = {
                val report = Buds.session.exportDebugReport()
                ctx.startActivity(
                    Intent.createChooser(
                        Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, report),
                        "Debug raporunu paylaş"
                    ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                )
            }) { Text("Rapor paylaş") }
            OutlinedButton(onClick = { Buds.session.clearLogs(); onRefresh() }) { Text("Temizle") }
        }
        if (diag.isEmpty()) Hint("Kayıt yok")
        diag.take(60).forEach { d ->
            Text("${fmt(d.tsMs)}  ${d.title}", style = MaterialTheme.typography.labelLarge)
            Hint(d.detail)
        }
        HorizontalDivider()
        SwitchRow("Ham paketleri göster", showPackets) { showPackets = it }
        if (showPackets) {
            SelectionContainer {
                Column {
                    packets.take(120).forEach { p ->
                        Text(
                            "${fmt(p.tsMs)} ${if (p.outgoing) "→" else "←"} ${p.name}\n${p.hex}",
                            fontFamily = FontFamily.Monospace,
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                }
            }
        }
        HorizontalDivider()
        OutlinedTextField(
            hex, { hex = it }, label = { Text("Hex paket çöz (ör. HCI log'dan)") },
            modifier = Modifier.fillMaxWidth(), textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace)
        )
        OutlinedButton(onClick = { decoded = decodeHex(hex) }) { Text("Çöz") }
        if (decoded.isNotEmpty()) {
            SelectionContainer { Text(decoded, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall) }
        }
    }
}
