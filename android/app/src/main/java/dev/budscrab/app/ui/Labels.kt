package dev.budscrab.app.ui

import dev.budscrab.core.AppAction
import dev.budscrab.core.EqPreset
import dev.budscrab.core.HoldMode
import dev.budscrab.core.NoiseControl
import dev.budscrab.core.Placement

fun NoiseControl.label(): String = when (this) {
    NoiseControl.OFF -> "Kapalı"
    NoiseControl.ANC -> "ANC"
    NoiseControl.AMBIENT -> "Ortam sesi"
}

fun EqPreset.label(): String = when (this) {
    EqPreset.OFF -> "Normal"
    EqPreset.BASS_BOOST -> "Bas güçlendirme"
    EqPreset.SOFT -> "Yumuşak"
    EqPreset.DYNAMIC -> "Dinamik"
    EqPreset.CLEAR -> "Net"
    EqPreset.TREBLE_BOOST -> "Tiz güçlendirme"
}

fun HoldMode.label(): String = when (this) {
    HoldMode.VOICE_ASSISTANT -> "Sesli asistan"
    HoldMode.NOISE_CONTROL -> "Gürültü kontrolü"
    HoldMode.VOLUME -> "Ses seviyesi"
    HoldMode.SPOTIFY -> "Spotify"
    HoldMode.CUSTOM -> "Özel (BudsCrab)"
    HoldMode.UNKNOWN -> "Bilinmiyor"
}

fun AppAction.label(): String = when (this) {
    AppAction.NOTHING -> "Hiçbir şey"
    AppAction.PLAY_PAUSE -> "Oynat / duraklat"
    AppAction.NEXT_TRACK -> "Sonraki parça"
    AppAction.PREVIOUS_TRACK -> "Önceki parça"
    AppAction.VOLUME_UP -> "Sesi aç"
    AppAction.VOLUME_DOWN -> "Sesi kıs"
    AppAction.CYCLE_NOISE_CONTROL -> "Gürültü modunu değiştir"
    AppAction.TOGGLE_ANC -> "ANC aç/kapa"
    AppAction.TOGGLE_AMBIENT -> "Ortam sesi aç/kapa"
    AppAction.VOICE_ASSISTANT -> "Sesli asistan"
}

fun Placement.label(): String = when (this) {
    Placement.UNKNOWN -> "?"
    Placement.DISCONNECTED -> "Bağlı değil"
    Placement.WEARING -> "Kulakta"
    Placement.IDLE -> "Dışarıda"
    Placement.IN_CASE -> "Kutuda"
    Placement.CLOSED_CASE -> "Kutu kapalı"
}

/** Hold modes the user may pick (Unknown is display-only). */
val pickableHoldModes = listOf(HoldMode.NOISE_CONTROL, HoldMode.VOLUME, HoldMode.VOICE_ASSISTANT, HoldMode.SPOTIFY, HoldMode.CUSTOM)
