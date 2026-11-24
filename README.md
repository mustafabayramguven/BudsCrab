# BudsCrab 🦀🎧

Samsung **Galaxy Buds Core** (SM-R410) için açık kaynak Android yöneticisi.
Google Play / Play Services gerektirmez; vanilla Android, /e/OS, GrapheneOS vb.
üzerinde çalışır. Protokol mantığı Rust'ta, arayüz Kotlin/Compose.

## Özellikler

- **Pano:** sol/sağ/kutu pili, şarj durumu, takılı mı / kutuda mı
- **Gürültü kontrolü:** ANC / ortam sesi / kapalı, ortam sesi seviyesi, güçlü ANC
- **Dokunmatik:** tek / çift / üçlü dokunma ve basılı tutmayı tek tek aç/kapat, dokunmatik kilidi
- **Tek dokunma koruması:** tek dokunuşu kapatıp oynat/duraklatı basılı tutmaya taşır (kazara durma biter)
- **Özel basılı tutma:** her kulaklıkta 1×, 2×, 3× basılı tutmaya ayrı aksiyon
  (oynat/duraklat, sonraki/önceki, ses, gürültü modu döngüsü, ANC/ortam aç-kapa, asistan)
- **"Ayarlarımı koru":** dokunma ayarlarını her bağlantıda kulaklığa yeniden yazar
- **Duraklatma dedektifi:** müzik durunca sebebini tahmin eder (dokunma, takma sensörü, bağlantı kopması, telefon)
- **Ekolayzır** ön ayarları, sol/sağ ses dengesi
- **Kulaklığımı bul** (tek tarafı susturma ile), **kulak uyumu testi**
- **Wi-Fi profilleri:** belirli ağa bağlanınca gürültü modu / EQ otomatik değişir
- **Pil geçmişi** grafiği (24 saat / 7 gün / 30 gün), düşük pil bildirimi
- **Hızlı ayarlar kutucuğu:** dokununca gürültü modu değişir
- **Bildirim:** pil + ANC / Ortam / Kapalı kısayolları
- **Tanılama:** ham paket günlüğü, debug raporu paylaşma, hex paket çözücü
- Kulaklık bağlanınca otomatik başlar

## Yapı

```
crates/
  budscrab-proto/   Saf Rust protokol: çerçeveleme, CRC16, mesaj decoder/encoder (bağımlılık yok)
  budscrab-core/    Oturum durumu, özel hareket motoru, profiller, geçmiş → UniFFI ile Kotlin'e
  budscrab-cli/     Hex/HCI log çözücü (tersine mühendislik için)
android/            Kotlin + Jetpack Compose uygulaması (RFCOMM soketi burada)
```

Kotlin soketi açar, gelen baytları `Session.feed()` ile Rust'a verir; Rust
`CoreEvent` listesi döner (gönderilecek baytlar, yeni durum, telefon aksiyonu).
Rust tarafında thread, I/O, zamanlayıcı yok; her şey test edilebilir.

## Derleme

**GitHub Actions:** her push'ta APK derlenir → Actions → son çalışma → `budscrab-apk`.
`v*` etiketi atınca Release'e de eklenir.

Güncellemeleri üstüne kurabilmek için sabit bir imza anahtarı ekle
(yoksa her CI derlemesi farklı debug anahtarıyla imzalanır):

```sh
keytool -genkeypair -v -keystore budscrab.jks -alias budscrab -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 budscrab.jks   # çıktıyı BUDSCRAB_KEYSTORE_B64 secret'ı yap
```
Secret'lar: `BUDSCRAB_KEYSTORE_B64`, `BUDSCRAB_KEYSTORE_PASSWORD`, `BUDSCRAB_KEY_ALIAS`, `BUDSCRAB_KEY_PASSWORD`.

**Yerelde:**

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=~/Android/Sdk/ndk/<sürüm>
./scripts/build-android.sh
cd android && ./gradlew assembleRelease
```

Testler: `cargo test --workspace`

## Tersine mühendislik

Buds Core'un mesaj düzeni upstream'de "Buds FE ile büyük ihtimalle aynı" diye
işaretli, gerçek cihazla doğrulanmamış. Doğrulamak için:

1. Geliştirici seçenekleri → **Bluetooth HCI snoop günlüğünü etkinleştir**
2. BudsCrab ile bağlan, ayarları değiştir
3. `adb bugreport` → `FS/data/misc/bluetooth/logs/btsnoop_hci.log`
4. `cargo run -p budscrab-cli -- scan btsnoop_hci.log`

Uygulama içinden de Daha → Tanılama → **Rapor paylaş** ile ham paketleri alabilirsin.

## Lisans ve teşekkür

GPL-3.0-only. Protokol bilgisi ve test verisi
[GalaxyBudsClient](https://github.com/timschneeb/GalaxyBudsClient) (Tim Schneeberger, GPL-3.0)
projesinden alınmıştır. Samsung ile bağlantısı yoktur.
