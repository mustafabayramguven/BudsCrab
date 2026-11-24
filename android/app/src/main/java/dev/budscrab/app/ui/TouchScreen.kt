package dev.budscrab.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.budscrab.app.Buds
import dev.budscrab.core.AppAction
import dev.budscrab.core.Config
import dev.budscrab.core.CustomHoldActions
import dev.budscrab.core.HoldMode
import dev.budscrab.core.NoiseCycleSet
import dev.budscrab.core.TouchSettings

@Composable
fun TouchScreen() {
    val st by Buds.state.collectAsStateWithLifecycle()
    val cfg by Buds.config.collectAsStateWithLifecycle()
    val s = st
    val c = cfg
    Column(Modifier.verticalScroll(rememberScrollState()).padding(vertical = 8.dp)) {
        if (s == null || !s.ready || c == null) {
            Section("Dokunmatik") { Text("Ayarları görmek için kulaklığa bağlan.") }
            return@Column
        }
        val t = s.touch ?: TouchSettings(false, true, true, true, true, true, true)
        fun setTouch(n: TouchSettings) = Buds.cmd { setTouch(n, it) }

        Section("Hareketler") {
            Hint("Kapattığın hareketi kulaklık tamamen yok sayar. Kazara duraklatma için “Tek dokunma”yı kapat.")
            SwitchRow("Dokunmatiği kilitle", t.locked, subtitle = "Tüm hareketler kapanır") { setTouch(t.copy(locked = it)) }
            HorizontalDivider()
            SwitchRow("Tek dokunma", t.singleTap, enabled = !t.locked, subtitle = "Oynat / duraklat") { setTouch(t.copy(singleTap = it)) }
            SwitchRow("Çift dokunma", t.doubleTap, enabled = !t.locked, subtitle = "Sonraki parça, aramayı cevapla") { setTouch(t.copy(doubleTap = it)) }
            SwitchRow("Üçlü dokunma", t.tripleTap, enabled = !t.locked, subtitle = "Önceki parça") { setTouch(t.copy(tripleTap = it)) }
            SwitchRow("Basılı tutma", t.hold, enabled = !t.locked) { setTouch(t.copy(hold = it)) }
            SwitchRow("Aramada çift dokunma", t.doubleTapCall, enabled = !t.locked) { setTouch(t.copy(doubleTapCall = it)) }
            SwitchRow("Aramada basılı tutma", t.holdCall, enabled = !t.locked, subtitle = "Aramayı reddet") { setTouch(t.copy(holdCall = it)) }
            HorizontalDivider()
            SwitchRow(
                "Ayarlarımı koru", c.enforceTouchSettings,
                subtitle = "Her bağlantıda bu hareket ayarlarını kulaklığa yeniden yaz"
            ) { on -> Buds.updateConfig { it.copy(enforceTouchSettings = on, savedTouch = it.savedTouch ?: t) } }
            if (t.singleTap) {
                Button(onClick = { Buds.cmd { applyTapGuard(it) } }) { Text("Tek dokunma korumasını uygula") }
            }
        }

        Section("Basılı tutma (kulaklık)") {
            val left = s.holdLeft ?: HoldMode.UNKNOWN
            val right = s.holdRight ?: HoldMode.UNKNOWN
            Picker("Sol", left, pickableHoldModes, { it.label() }) { m -> Buds.cmd { setHoldModes(m, right, it) } }
            Picker("Sağ", right, pickableHoldModes, { it.label() }) { m -> Buds.cmd { setHoldModes(left, m, it) } }
            Hint("“Özel” seçersen kulaklık işi BudsCrab'a bırakır; aşağıdan 1×, 2×, 3× basılı tutmaya istediğin aksiyonu ver.")
            if (left == HoldMode.NOISE_CONTROL || right == HoldMode.NOISE_CONTROL) {
                NoiseCycleEditor(s.cycleLeft ?: NoiseCycleSet(true, true, false), s.cycleRight ?: NoiseCycleSet(true, true, false))
            }
        }

        if (s.holdLeft == HoldMode.CUSTOM || s.holdRight == HoldMode.CUSTOM) {
            CustomHoldEditor(c, s.holdLeft == HoldMode.CUSTOM, s.holdRight == HoldMode.CUSTOM)
        }

        s.outsideDoubleTap?.let { odt ->
            Section("Kenar dokunuşu") {
                SwitchRow("Kenara çift dokunarak ses ayarı", odt) { on -> Buds.cmd { setOutsideDoubleTap(on, it) } }
            }
        }
    }
}

@Composable
private fun NoiseCycleEditor(left: NoiseCycleSet, right: NoiseCycleSet) {
    Text("Basılı tutunca dolaşılacak modlar", style = MaterialTheme.typography.titleSmall)
    @Composable
    fun row(name: String, set: NoiseCycleSet, update: (NoiseCycleSet) -> Unit) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(name, Modifier.padding(top = 12.dp, end = 4.dp))
            FilterChip(set.anc, { update(set.copy(anc = !set.anc)) }, { Text("ANC") })
            FilterChip(set.ambient, { update(set.copy(ambient = !set.ambient)) }, { Text("Ortam") })
            FilterChip(set.off, { update(set.copy(off = !set.off)) }, { Text("Kapalı") })
        }
    }
    row("Sol", left) { n -> Buds.cmd { setNoiseCycle(n, right, it) } }
    row("Sağ", right) { n -> Buds.cmd { setNoiseCycle(left, n, it) } }
    Hint("Her tarafta en az iki mod seçili olmalı.")
}

@Composable
private fun CustomHoldEditor(c: Config, leftOn: Boolean, rightOn: Boolean) {
    val h = c.customHolds
    val actions = AppAction.entries
    fun upd(f: (CustomHoldActions) -> CustomHoldActions) = Buds.updateConfig { it.copy(customHolds = f(it.customHolds)) }
    Section("Özel basılı tutma aksiyonları") {
        if (leftOn) {
            Text("Sol", style = MaterialTheme.typography.titleSmall)
            Picker("1× basılı tut", h.leftHold, actions, { it.label() }) { a -> upd { it.copy(leftHold = a) } }
            Picker("2× basılı tut", h.leftDoubleHold, actions, { it.label() }) { a -> upd { it.copy(leftDoubleHold = a) } }
            Picker("3× basılı tut", h.leftTripleHold, actions, { it.label() }) { a -> upd { it.copy(leftTripleHold = a) } }
        }
        if (rightOn) {
            Text("Sağ", style = MaterialTheme.typography.titleSmall)
            Picker("1× basılı tut", h.rightHold, actions, { it.label() }) { a -> upd { it.copy(rightHold = a) } }
            Picker("2× basılı tut", h.rightDoubleHold, actions, { it.label() }) { a -> upd { it.copy(rightDoubleHold = a) } }
            Picker("3× basılı tut", h.rightTripleHold, actions, { it.label() }) { a -> upd { it.copy(rightTripleHold = a) } }
        }
        var w by remember(h.multiHoldWindowMs) { mutableFloatStateOf(h.multiHoldWindowMs.toFloat()) }
        Text("Art arda basılı tutma süresi: ${w.toInt()} ms")
        Slider(
            value = w, onValueChange = { w = it }, valueRange = 600f..2500f,
            onValueChangeFinished = { upd { it.copy(multiHoldWindowMs = w.toLong()) } }
        )
        Hint("2× / 3× tanımlıysa BudsCrab ilk basılı tutmadan sonra bu süre kadar bekler. Sadece 1× tanımlıysa aksiyon anında çalışır.")
        val cyc = c.appNoiseCycle
        Text("“Gürültü modunu değiştir” sırası", style = MaterialTheme.typography.titleSmall)
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FilterChip(cyc.anc, { Buds.updateConfig { it.copy(appNoiseCycle = cyc.copy(anc = !cyc.anc)) } }, { Text("ANC") })
            FilterChip(cyc.ambient, { Buds.updateConfig { it.copy(appNoiseCycle = cyc.copy(ambient = !cyc.ambient)) } }, { Text("Ortam") })
            FilterChip(cyc.off, { Buds.updateConfig { it.copy(appNoiseCycle = cyc.copy(off = !cyc.off)) } }, { Text("Kapalı") })
        }
    }
}
