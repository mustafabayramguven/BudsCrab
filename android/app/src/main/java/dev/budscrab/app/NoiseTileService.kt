package dev.budscrab.app

import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import dev.budscrab.app.ui.label
import dev.budscrab.core.AppAction
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

/** Quick settings tile: tap to cycle ANC / ambient / off. */
class NoiseTileService : TileService() {
    private var job: Job? = null

    override fun onStartListening() {
        super.onStartListening()
        job = CoroutineScope(Dispatchers.Main).launch {
            Buds.state.collectLatest { refresh() }
        }
    }

    override fun onStopListening() {
        job?.cancel()
        super.onStopListening()
    }

    override fun onClick() {
        super.onClick()
        if (Buds.isConnected) {
            Buds.perform(AppAction.CYCLE_NOISE_CONTROL)
        } else {
            runCatching { Buds.startService(this) }
        }
        refresh()
    }

    private fun refresh() {
        val tile = qsTile ?: return
        val s = Buds.state.value
        if (Buds.isConnected && s?.ready == true) {
            tile.state = Tile.STATE_ACTIVE
            tile.label = s.noiseControl?.label() ?: "Gürültü"
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = "Sol %${s.batteryLeft ?: "-"} · Sağ %${s.batteryRight ?: "-"}"
        } else {
            tile.state = Tile.STATE_INACTIVE
            tile.label = "BudsCrab"
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = "Bağlı değil"
        }
        tile.updateTile()
    }
}
