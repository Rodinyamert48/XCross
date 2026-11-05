# XCross

Kendi nişangâhını (crosshair) göstermeyen oyunlar için ekranın ortasına
özelleştirilebilir bir crosshair çizen, Rust ile yazılmış overlay.

Pencere tam ekranı kaplar, saydamdır, her zaman üsttedir ve **tıklama
geçirgendir** (click-through) — yani fare olaylarını altındaki oyuna iletir.

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

## Kurulum

```bash
cargo build --release
```

Çalıştırılabilir dosya: `target/release/xcross`.

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

## Bilinen sınırlamalar

- **Tam ekran (exclusive fullscreen) oyunlar:** hiçbir overlay bu modda
  görünmez. Oyunu *kenarlıksız pencere* (borderless windowed) modunda
  çalıştırın.
- **Linux/X11:** saydamlık için çalışan bir birleştirici (compositor)
  gerekir; yoksa overlay siyah bir dikdörtgen olarak görünür.
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
