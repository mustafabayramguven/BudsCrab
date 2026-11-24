package dev.budscrab.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.budscrab.app.Buds
import dev.budscrab.core.EqPreset

@Composable
fun SoundScreen() {
    val st by Buds.state.collectAsStateWithLifecycle()
    val s = st
    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
        if (s == null || !s.ready) {
            Section("Ses") { Text("Ayarları görmek için kulaklığa bağlan.") }
            return@Column
        }
        Section("Ekolayzır") {
            EqPreset.entries.forEach { p ->
                Row(
                    Modifier.fillMaxWidth().clickable { Buds.cmd { setEq(p, it) } },
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    RadioButton(selected = s.eq == p, onClick = { Buds.cmd { setEq(p, it) } })
                    Text(p.label())
                }
            }
        }
        s.stereoBalance?.let { bal ->
            Section("Sol / sağ dengesi") {
                var v by remember(bal) { mutableFloatStateOf(bal.toFloat()) }
                val diff = v.toInt() - 16
                Text(
                    when {
                        diff < 0 -> "Sola ${-diff}"
                        diff > 0 -> "Sağa $diff"
                        else -> "Ortada"
                    }
                )
                Slider(
                    value = v, onValueChange = { v = it }, valueRange = 0f..32f, steps = 31,
                    onValueChangeFinished = { Buds.cmd { setStereoBalance(v.toInt(), it) } }
                )
            }
        }
    }
}
