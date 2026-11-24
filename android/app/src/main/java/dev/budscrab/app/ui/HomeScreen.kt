package dev.budscrab.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.BatteryChargingFull
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.budscrab.app.Buds
import dev.budscrab.app.ConnState
import dev.budscrab.core.DeviceState
import dev.budscrab.core.NoiseControl
import dev.budscrab.core.Placement

@Composable
fun HomeScreen() {
    val st by Buds.state.collectAsStateWithLifecycle()
    val conn by Buds.conn.collectAsStateWithLifecycle()
    val lastPause by Buds.lastPause.collectAsStateWithLifecycle()
    val ctx = LocalContext.current
    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
        Section("Bağlantı") {
            val text = when (val c = conn) {
                is ConnState.Connected -> "Bağlı: ${c.name}"
                is ConnState.Connecting -> "Bağlanıyor: ${c.name}…"
                is ConnState.Waiting -> c.reason
                ConnState.Idle -> "Bağlı değil" + (Buds.prefs.deviceName?.let { " ($it)" } ?: "")
            }
            Text(text, style = MaterialTheme.typography.bodyLarge)
            st?.modelSku?.let { Hint("Model: $it") }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (conn is ConnState.Idle) {
                    Button(onClick = { Buds.startService(ctx) }) { Text("Bağlan") }
                } else {
                    OutlinedButton(onClick = { Buds.stopService(ctx) }) { Text("Bağlantıyı kes") }
                }
            }
        }
        val s = st
        if (s != null && s.ready) {
            if (s.touch?.singleTap == true && s.touch?.locked == false) TapGuardBanner()
            BatterySection(s)
            NoiseSection(s)
            lastPause?.let { reason ->
                Section("Son duraklatma") {
                    Text(reason)
                    Hint("Duraklatma dedektifi müzik her durduğunda sebebini tahmin eder. Ayrıntı: Daha → Tanılama")
                }
            }
        }
    }
}

@Composable
private fun TapGuardBanner() {
    Card(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Filled.Warning, contentDescription = null)
                Text("  Müzik kendi kendine mi duruyor?", fontWeight = FontWeight.Bold)
            }
            Text(
                "Tek dokunma açık. Kulaklığı düzeltirken hafif bir temas bile müziği durdurur. " +
                    "Tek dokunmayı kapatıp oynat/duraklatı sağ kulaklığı basılı tutmaya taşıyabilirim."
            )
            Button(onClick = { Buds.cmd { applyTapGuard(it) } }) { Text("Tek dokunma korumasını aç") }
        }
    }
}

@Composable
private fun BatterySection(s: DeviceState) {
    Section("Pil") {
        BatteryRow("Sol", s.batteryLeft, s.chargingLeft, s.placementLeft)
        BatteryRow("Sağ", s.batteryRight, s.chargingRight, s.placementRight)
        if ((s.batteryCase ?: 0) > 0 || s.chargingCase) BatteryRow("Kutu", s.batteryCase, s.chargingCase, null)
    }
}

@Composable
private fun BatteryRow(name: String, level: Int?, charging: Boolean, placement: Placement?) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Text(name, Modifier.padding(end = 12.dp), fontWeight = FontWeight.SemiBold)
        LinearProgressIndicator(
            progress = { (level ?: 0) / 100f },
            modifier = Modifier.weight(1f)
        )
        Text("  %${level ?: "-"}")
        if (charging) Icon(Icons.Filled.BatteryChargingFull, contentDescription = "Şarj oluyor")
    }
    if (placement != null) Hint(placement.label())
}

@Composable
private fun NoiseSection(s: DeviceState) {
    Section("Gürültü kontrolü") {
        val modes = listOf(NoiseControl.ANC, NoiseControl.AMBIENT, NoiseControl.OFF)
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
            modes.forEachIndexed { i, m ->
                SegmentedButton(
                    selected = s.noiseControl == m,
                    onClick = { Buds.cmd { setNoiseControl(m, it) } },
                    shape = SegmentedButtonDefaults.itemShape(i, modes.size)
                ) { Text(m.label()) }
            }
        }
        if (s.noiseControl == NoiseControl.AMBIENT) {
            var v by remember(s.ambientVolume) { mutableFloatStateOf((s.ambientVolume ?: 1).toFloat()) }
            Text("Ortam sesi seviyesi: ${v.toInt() + 1}/3")
            Slider(
                value = v, onValueChange = { v = it }, valueRange = 0f..2f, steps = 1,
                onValueChangeFinished = { Buds.cmd { setAmbientVolume(v.toInt(), it) } }
            )
        }
        if (s.noiseControl == NoiseControl.ANC && s.noiseReductionHigh != null) {
            SwitchRow("Güçlü ANC", s.noiseReductionHigh == true) { on -> Buds.cmd { setNoiseReductionHigh(on, it) } }
        }
    }
}
