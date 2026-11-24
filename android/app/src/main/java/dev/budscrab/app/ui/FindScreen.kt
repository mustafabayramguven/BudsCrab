package dev.budscrab.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.budscrab.app.Buds

@Composable
fun FindScreen() {
    val st by Buds.state.collectAsStateWithLifecycle()
    val s = st
    DisposableEffect(Unit) {
        onDispose { if (Buds.state.value?.finding == true) Buds.cmd { find(false, it) } }
    }
    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
        if (s == null || !s.ready) {
            Section("Kulaklığımı bul") { Text("Kulaklık bağlı değilse çalmaz; son bağlantıdan beri kutudaysa kutuyu aç.") }
            return@Column
        }
        Section("Kulaklığımı bul") {
            Hint("Kulaklıklar yüksek sesle bip sesi çalar. Kulağındayken başlatma!")
            if (!s.finding) {
                Button(onClick = { Buds.cmd { find(true, it) } }) { Text("Çalmaya başla") }
            } else {
                OutlinedButton(onClick = { Buds.cmd { find(false, it) } }) { Text("Durdur") }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(s.mutedLeft, { Buds.cmd { muteEarbuds(!s.mutedLeft, s.mutedRight, it) } }, { Text("Solu sustur") })
                    FilterChip(s.mutedRight, { Buds.cmd { muteEarbuds(s.mutedLeft, !s.mutedRight, it) } }, { Text("Sağı sustur") })
                }
            }
        }
        Section("Kulak uyumu testi") {
            Hint("Kulaklıkları tak, sessiz bir yerde testi başlat. Uç boyutunun doğru olup olmadığını gösterir.")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { Buds.cmd { fitTest(true, it) } }) { Text("Testi başlat") }
                OutlinedButton(onClick = { Buds.cmd { fitTest(false, it) } }) { Text("Durdur") }
            }
            if (s.fitLeft != null || s.fitRight != null) {
                Text("Sol: ${s.fitLeft ?: "-"}   Sağ: ${s.fitRight ?: "-"}")
            }
        }
    }
}
