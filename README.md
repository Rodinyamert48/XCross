# XCross

Kendi nişangâhını (crosshair) göstermeyen oyunlar için ekranın ortasına
özelleştirilebilir bir crosshair çizen, Rust ile yazılmış overlay.

Crosshair, ekranın ortasında duran küçük, saydam, her zaman üstte ve
**tıklama geçirgen** (click-through) bir pencerede çizilir — fare ve klavye
olayları doğrudan oyuna gider, oyunun odağı hiç çalınmaz.

## Özellikler

- **Şekiller:** artı, nokta, daire, T biçimi, X biçimi, artı + daire
- **Geometri:** kol uzunluğu, kalınlık, merkez boşluğu, dönüş açısı, daire
  yarıçapı (dolu/boş), merkez noktası
- **Renk:** ana renk, merkez noktası rengi, kontur (dış çizgi) rengi ve
  kalınlığı, kanal başına saydamlık, genel opaklık
- **Konum:** yatay/dikey kayma, piksele hizalama
- **Profiller:** istediğin kadar ayar seti; kopyala, sil, aralarında geçiş yap
- **Global kısayollar:** oyun odaktayken bile göster/gizle, profil değiştir,
  ayarları aç/kapat
- **Canlı önizleme** ve otomatik kaydetme
- **Sistem tepsisi simgesi** (Windows): ayarlar penceresi kapatılınca program
  arka planda çalışmaya devam eder; tepsiden geri açılır veya kapatılır

## Kurulum

Hazır Windows sürümü: [Releases](../../releases) sayfasından
`xcross-windows-x86_64.zip` dosyasını indirip açın, `xcross.exe`'yi çalıştırın.
Kurulum gerekmez. İmzasız olduğu için SmartScreen uyarı verebilir:
**Ek bilgi → Yine de çalıştır**.

Kaynaktan derlemek için:

```bash
cargo build --release
```

Çalıştırılabilir dosya: `target/release/xcross` (Windows'ta `xcross.exe`).

### Linux bağımlılıkları

X11/Wayland geliştirme kitaplıkları gerekir, örn. Debian/Ubuntu'da:

```bash
sudo apt install libxkbcommon-x11-0 libgl1-mesa-dri
```

## Kullanım

```bash
xcross                      # ayarlar penceresiyle başlar
xcross --no-settings        # yalnızca overlay
xcross --profile valorant   # belirli bir profille başla
xcross --config ./my.toml   # alternatif konfigürasyon dosyası
```

### Ayarlar penceresi ve çıkış

- **Arka plana al** (veya pencerenin X düğmesi): ayarları gizler, crosshair
  çalışmaya devam eder.
- **Çıkış**: programı tamamen kapatır.
- Gizliyken geri açmak için: `Alt+S` ya da saatin yanındaki tepsi simgesine
  tıklayın. Tepsi simgesine sağ tıklayınca *Ayarlar / Crosshair göster-gizle /
  Çıkış* menüsü açılır.
- Ayarların üstündeki küçük durum satırı overlay'in çalışıp çalışmadığını,
  crosshair'in hangi ekran koordinatına çizildiğini gösterir. Bir sorun
  olduğunda turuncu yazar.

### Varsayılan kısayollar

| Kısayol   | İşlev                    |
|-----------|--------------------------|
| `Alt+X`   | Crosshair'i göster/gizle |
| `Alt+C`   | Sıradaki profile geç     |
| `Alt+S`   | Ayarlar penceresi        |

Kısayollar ayarlardan değiştirilebilir. Yazım biçimi
`Ctrl+Alt+KeyX`, `Shift+F1`, `Alt+Numpad5` şeklindedir
([`global-hotkey`](https://docs.rs/global-hotkey) söz dizimi).

## Konfigürasyon

Ayarlar TOML olarak saklanır; yol ayarlar penceresinin "Genel" bölümünde
yazar. Tipik konumlar:

- Windows: `%APPDATA%\XCross\config\config.toml`
- Linux: `~/.config/XCross/config.toml`
- macOS: `~/Library/Application Support/XCross/config.toml`

```toml
active_profile = "default"
autosave = true

[profiles.default]
style = "cross"
arm_length = 10.0
thickness = 2.0
gap = 4.0
color = { rgb = [0, 255, 0], alpha = 255 }
outline = true
```

Renkte alfa, `rgb` bileşenlerinden ayrı tutulur: renk seçicileri alfayı
önceden çarptığı için tek bir dörtlüde düzenleme, düşük saydamlıkta renk
tonunu bozar.

## Nasıl çalışır (Windows)

Overlay, yalnızca crosshair kadar büyük, katmanlı (`WS_EX_LAYERED`), tıklama
geçirgen (`WS_EX_TRANSPARENT`), odak almayan ve en üstte (`WS_EX_TOPMOST`)
duran yerel bir Win32 penceresidir. İçerik CPU'da çizilir ve piksel bazında
saydamlıkla `UpdateLayeredWindow` ile ekrana basılır. Oyun kendini öne
aldığında overlay hem ön plan değişimi anında hem de 250 ms'de bir yeniden
en üste alınır.

Ayarlar penceresi egui ile çizilir. İlk sürümde crosshair da bir egui
penceresiydi; winit'in tıklama geçirgenliği için eklediği `WS_EX_LAYERED`
bayrağından sonra `SetLayeredWindowAttributes` çağrılmadığı için Windows bu
pencereyi hiç çizmiyordu.

## Bilinen sınırlamalar

- **Tam ekran (exclusive fullscreen) oyunlar:** hiçbir overlay bu modda
  görünmez. Oyunu *kenarlıksız pencere* (borderless windowed / fullscreen
  window) modunda çalıştırın.
- **Ekran ölçekleme:** crosshair ölçüleri gerçek ekran pikselidir; %125 /
  %150 ölçeklemede de aynı boyutta ve keskin kalır.
- **Linux/X11:** overlay saydam bir egui penceresidir; saydamlık için
  çalışan bir birleştirici (compositor) gerekir, yoksa siyah bir dikdörtgen
  olarak görünür. Tepsi simgesi yalnızca Windows'ta vardır.
- **Wayland:** "her zaman üstte" ve global kısayollar protokol gereği
  kısıtlıdır; kısayollar devre dışı kalırsa ayarlar penceresinde uyarı çıkar.
- **Çok monitörlü sistemler:** overlay varsayılan olarak birincil monitörü
  kaplar. Başka bir ekran için ayarlardaki "Overlay penceresi → Elle
  konumlandır" bölümünü kullanın.
- Rekabetçi oyunların anti-cheat sistemleri harici overlay'lere farklı
  tepkiler verir; kullanmadan önce ilgili oyunun kurallarını kontrol edin.

## Geliştirme

```bash
cargo test
cargo clippy --all-targets
```
