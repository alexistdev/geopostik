# Arsitektur & Struktur Proyek GeoPOSTik

Turunan dari [FLOW.md](FLOW.md) dan [SCHEMA.md](SCHEMA.md).

## 1. Gambaran

GeoPOSTik adalah **satu aplikasi desktop** (satu file `.exe`). Tidak ada server terpisah,
tidak ada port HTTP. Frontend dan backend berjalan dalam satu proses:

```
┌──────────────────────── GeoPOSTik.exe (Tauri) ────────────────────────┐
│                                                                        │
│  Frontend: Angular + PrimeNG            Backend: Rust                  │
│  (dirender WebView2)                                                   │
│                                                                        │
│  komponen ──► service Angular ── invoke("sale_create", {...}) ──►  command
│                                  (IPC Tauri, bukan HTTP)            │  │
│                                                                     ▼  │
│  tampilan ◄── hasil / AppError { code, message } ◄──────────── service │
│                                                                     │  │
│                                                                   repo │
│                                                                     │  │
└─────────────────────────────────────────────────────────────────────┼──┘
                                                                      ▼
                                          %APPDATA%\GeoPOSTik\geopostik.db (SQLite)
```

## 2. Versi (per September 2026)

| Komponen | Versi |
|---|---|
| Tauri | 2.x (stabil, CLI 2.11). Tauri 3 masih alpha, tidak dipakai |
| Angular | **21.x** (standalone components + signals, zoneless, tanpa NgModule) |
| PrimeNG | **21.x, dikunci di bawah 22** |
| PrimeIcons | **7.x, dikunci di bawah 8** |
| Rust | stable (edisi 2024) |
| Node.js | 22 / 24 / 26 (untuk build Angular saja; tidak ikut ke aplikasi jadi) |
| SQLite | ikut ter-compile lewat `rusqlite` fitur `bundled` (FTS5 aktif) |

**Kenapa bukan Angular 22 + PrimeNG 22?** Mulai PrimeNG 22 dan PrimeIcons 8, lisensinya
berubah dari MIT menjadi "PrimeUI License" yang butuh license key (Community gratis harus
diperpanjang tiap tahun). Tanpa key yang valid muncul banner "Invalid PrimeUI License",
yang berisiko tampil di komputer kasir klien bila key kedaluwarsa. PrimeNG 21, PrimeIcons 7,
dan paket `@primeuix/*` yang dipakainya masih MIT. PrimeNG 21 hanya mendukung Angular 21,
sehingga Angular ikut di versi 21. **Jangan menaikkan ke PrimeNG ≥ 22 / PrimeIcons ≥ 8**
tanpa memutuskan ulang soal lisensi.

## 3. Struktur folder

Satu repo. Frontend dan backend dipisah folder, mengikuti konvensi Tauri:
**Angular di root, Rust di `src-tauri/`**.

```
geopostik/
├── docs/                          # FLOW, SCHEMA, ARCHITECTURE
├── package.json, angular.json     # ── FRONTEND (Angular) ──
├── src/
│   ├── main.ts, index.html, styles.scss
│   └── app/
│       ├── app.config.ts, app.routes.ts
│       ├── core/                  # api/ (wrapper invoke per domain), auth guard,
│       │                          # permission service, hotkey service, error handler
│       ├── shared/                # komponen umum: input rupiah, dialog PIN,
│       │                          # pencarian obat, pipe format rupiah/tanggal
│       ├── bindings/              # tipe TS hasil generate ts-rs (JANGAN diedit manual)
│       └── features/              # lazy-loaded per menu
│           ├── auth/              # login, kunci layar
│           ├── dashboard/
│           ├── pos/               # kasir, shift
│           ├── master/            # obat, satuan, harga, supplier, dokter, pelanggan, user
│           ├── purchasing/        # penerimaan barang, hutang
│           ├── inventory/         # stok awal, kartu stok, opname, pemusnahan
│           ├── reports/
│           └── settings/          # profil, pajak & harga, printer, backup, akses
│
└── src-tauri/                     # ── BACKEND (Rust) ──
    ├── Cargo.toml, tauri.conf.json, build.rs
    ├── icons/, capabilities/
    ├── migrations/                # 001_init.sql, 002_....sql (tertanam di binary)
    ├── tests/                     # integration test service dengan SQLite in-memory
    └── src/
        ├── main.rs                # entry point
        ├── lib.rs                 # setup Tauri, plugin, state, daftar command
        ├── state.rs               # AppState: koneksi DB, sesi login
        ├── error.rs               # AppError → { code, message }
        ├── money.rs               # Rupiah(i64), CostX100(i64), Bp(i64)
        ├── db/                    # koneksi, pragma, migration, helper transaksi
        ├── auth/                  # login, sesi, Role, Permission, PIN otorisasi
        ├── settings/
        ├── audit/
        ├── master/                # products, units, prices, price_tiers, suppliers, ...
        ├── inventory/             # batches, stock_movements, FEFO, opname
        ├── purchasing/
        ├── sales/                 # shift, sale, payment, void
        ├── reports/
        ├── printing/              # layout struk, ESC/POS, transport printer
        └── backup/
```

Setiap modul domain di Rust punya tiga lapis:

| File | Isi | Aturan |
|---|---|---|
| `commands.rs` | fungsi `#[tauri::command]` | tipis: ambil sesi, cek permission, panggil service |
| `service.rs` | logika bisnis | batas transaksi DB ada di sini; tidak tahu soal Tauri → mudah di-test |
| `repo.rs` | SQL | tidak ada logika bisnis |
| `model.rs` | struct input/output | `#[derive(Serialize, Deserialize, TS)]` → tipe TS otomatis |

## 4. Library

| Kebutuhan | Rust | Angular |
|---|---|---|
| Database | `rusqlite` (bundled), `rusqlite_migration` | – |
| Tipe bersama | `ts-rs` → `src/app/bindings/` | – |
| Error | `thiserror`, `serde` | – |
| Password & PIN | `argon2` | – |
| Waktu | `chrono` | – |
| Excel | `calamine` (import), `rust_xlsxwriter` (export) | – |
| Printer | ESC/POS dibuat sendiri + crate `windows` (spooler) & `serialport` | – |
| Log | `tracing`, `tracing-appender` (file harian) | – |
| Plugin Tauri | `single-instance`, `dialog` (pilih file/folder), `fs` terbatas | `@tauri-apps/api` |
| UI | – | PrimeNG 21 + PrimeIcons 7, tema preset Aura (warna utama emerald) |

## 5. Prinsip

1. **Semua aturan bisnis dan cek hak akses di Rust.** Angular hanya menampilkan dan
   menyembunyikan menu sesuai permission, tapi keputusan akhir tetap di command Rust.
2. **Sesi login disimpan di `AppState` Rust.** Angular tidak menyimpan token; setiap
   command membaca user aktif dari state.
3. **Data sensitif difilter di Rust:** untuk user tanpa permission `VIEW_COST`, field HPP
   dan laba **tidak dikirim** sama sekali ke frontend (bukan hanya disembunyikan di UI).
4. **Koneksi DB:** satu koneksi tulis di balik `Mutex` + satu koneksi baca untuk laporan.
   WAL mode sehingga laporan berat tidak menahan kasir.
5. **Uang tidak pernah float**, baik di Rust (`Rupiah(i64)`) maupun di TypeScript
   (`number` bulat; format tampilan lewat pipe).
6. **Lokasi data:** `%APPDATA%\GeoPOSTik\` berisi `geopostik.db`, `backups\`, `logs\`,
   terpisah dari folder instalasi sehingga aman saat update aplikasi.

## 6. Printer struk

Pengaturan disimpan di `settings` key `printer.receipt`, dengan tombol **Cetak Tes**.

| Pengaturan | Pilihan |
|---|---|
| Lebar kertas | **58 mm** (default 32 karakter/baris) / **80 mm** (default 48 karakter/baris); jumlah karakter bisa diubah manual (misal 42) |
| Koneksi | **Printer Windows** (raw ESC/POS lewat spooler, untuk printer USB yang sudah ter-install driver) / **Jaringan** (IP:9100) / **Serial** (COM port + baud rate) |
| Pemotong kertas | ada / tidak (banyak printer 58 mm tanpa cutter → cukup feed beberapa baris) |
| Laci uang | tidak ada / pin 2 / pin 5 |
| Jumlah salinan | 1–3 |
| Header & footer | nama apotek, alamat, SIA, telepon, pesan penutup |

Layout struk dibuat dari model teks per baris (kiri/kanan/tengah, tebal, garis pemisah)
yang dirender sesuai jumlah karakter per baris, lalu diubah ke byte ESC/POS. Dengan begitu
satu layout otomatis cocok untuk 58 mm maupun 80 mm, dan bisa dites tanpa printer
(render ke teks).

Etiket resep (Tahap 3) memakai printer label terpisah, dirancang nanti.

## 7. License

License didapat dari server GeoLicense (https://geolicense.my.id/, SKU produk `GEOPOS`).
Karena aplikasi berjalan offline, server hanya dihubungi saat **aktivasi pertama** dan saat
**validasi ulang** (`POST /api/v1/licenses/activate`; aktivasi ulang di komputer yang sama
tidak memakai seat baru).

- Saat pertama dibuka, halaman **Aktivasi** muncul sebelum setup awal. Tanpa license valid
  aplikasi tidak bisa dipakai.
- Hasil aktivasi disimpan di `%APPDATA%\GeoPOSTik\license.json`: license key, masa aktif, ID
  komputer (hash dari MachineGuid), dan waktu pemakaian terakhir, ditandatangani HMAC-SHA256.
  File yang diubah atau disalin ke komputer lain dianggap tidak ada.
- Setelah masa aktif habis, atau jam komputer dimundurkan dari pemakaian terakhir, semua menu
  selain **Pengaturan** diganti kotak license dan command Rust menolak dengan
  `LICENSE_REQUIRED`. Pengguna menghubungkan komputer ke internet lalu klik **Validasi Ulang**.
- Kode: `src-tauri/src/license/` (Rust), `features/license/` dan `features/settings/` (Angular).

## 8. Pengembangan & build

- **Pengembangan** bisa di macOS (`npm run tauri dev`): UI dan logika bisa dikerjakan penuh.
  Printer Windows dan WebView2 hanya bisa dites di Windows.
- **Build installer** `.msi`/`.exe` **harus di Windows** (Tauri tidak mendukung cross-compile
  installer dari macOS). Pilihan: PC/VM Windows, atau GitHub Actions runner `windows-latest`.
- **Git:** langsung ke `main`.
- **Test:** integration test Rust di `src-tauri/tests/` untuk service kritis: FEFO pecah batch,
  stok minus ditolak, void mengembalikan stok ke batch asal, pemilihan tier harga, validasi
  pembayaran split, permission/HPP tersembunyi.
