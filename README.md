# GeoPOSTik

Aplikasi POS (Point of Sale) untuk apotek yang berjalan di Windows 11 **tanpa internet**.
Internet hanya dibutuhkan sekali saat aktivasi license, dan saat license diperpanjang.

Dibangun dengan Tauri 2 (desktop), Angular 21 + PrimeNG (tampilan), Rust (backend), dan
SQLite (database lokal).

---

## Daftar isi

1. [Persiapan](#1-persiapan)
2. [Clone & install dependency](#2-clone--install-dependency)
3. [Menjalankan aplikasi](#3-menjalankan-aplikasi)
4. [Mendapatkan license di GeoLicense](#4-mendapatkan-license-di-geolicense)
5. [Aktivasi & setup awal](#5-aktivasi--setup-awal)
6. [Membuat installer Windows](#6-membuat-installer-windows)
7. [Memperpanjang atau mengganti license](#7-memperpanjang-atau-mengganti-license)
8. [Masalah umum](#8-masalah-umum)

---

## 1. Persiapan

Pasang dulu perangkat berikut:

| Kebutuhan | Versi | Keterangan |
|---|---|---|
| [Git](https://git-scm.com/downloads) | terbaru | untuk clone repo |
| [Node.js](https://nodejs.org/) | 22 / 24 (LTS) | untuk build tampilan Angular |
| [Rust](https://rustup.rs/) | stable ≥ 1.95 | untuk backend; pasang lewat `rustup` |
| Prasyarat Tauri | – | lihat di bawah, sesuai sistem operasi |

**Windows 10/11**
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/),
  centang **"Desktop development with C++"**.
- WebView2 sudah ada di Windows 11. Di Windows 10, pasang
  [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/).

**macOS** (untuk pengembangan saja)
```bash
xcode-select --install
```

**Linux**: ikuti [prasyarat Tauri untuk Linux](https://v2.tauri.app/start/prerequisites/#linux).

Cek hasil pemasangan:
```bash
node -v
```
```bash
cargo --version
```

## 2. Clone & install dependency

```bash
git clone https://github.com/alexistdev/geopostik.git
```
```bash
cd geopostik
```
```bash
npm install
```

Dependency Rust diunduh otomatis saat aplikasi pertama kali dijalankan.

## 3. Menjalankan aplikasi

```bash
npm run tauri dev
```

Build pertama bisa memakan waktu beberapa menit karena Rust meng-compile semua library.
Setelah itu jendela **GeoPOSTik** terbuka dan menampilkan halaman **Aktivasi GeoPOSTik**.

> Jangan membuka `http://localhost:4200` di browser biasa. Tampilan memang muncul, tetapi
> tidak terhubung ke backend. Selalu pakai jendela yang dibuka oleh `npm run tauri dev`.

## 4. Mendapatkan license di GeoLicense

License GeoPOSTik dikelola di **[https://geolicense.my.id/](https://geolicense.my.id/)**.

1. Buka [https://geolicense.my.id/register](https://geolicense.my.id/register) lalu buat akun.
   Kalau sudah punya akun, [login](https://geolicense.my.id/login).
2. Buka menu **Marketplace** lalu pilih produk **GeoPOSTik** (SKU `GEOPOS`).
3. Pilih paket license lalu klik **Order this plan**.
   - **Paket gratis:** license langsung aktif. Satu akun hanya bisa mengambil satu license
     gratis per produk.
   - **Paket berbayar:** buka menu **Invoices**, bayar lewat transfer bank sesuai nominal
     tagihan, lalu tunggu admin memverifikasi pembayaran.
4. Buka menu **License**. Salin **license key** Anda, formatnya seperti
   `GEOLIC-XXXXXXXX-XXXXXXXX`.

Setiap license punya batas jumlah komputer (seat) dan masa aktif. Satu komputer memakai satu
seat. Aktivasi ulang di komputer yang sama tidak memakai seat baru.

## 5. Aktivasi & setup awal

1. **Hubungkan komputer ke internet.**
2. Di halaman **Aktivasi GeoPOSTik**, tempel license key lalu klik **Aktifkan**.
   Kalau license ditolak, pesan penyebabnya muncul (key tidak ditemukan, seat penuh, masa aktif
   habis, dan lain-lain), dan aplikasi belum bisa dipakai.
3. Kalau berhasil, aplikasi membuat file license lokal dan membuka halaman **Setup awal**.
   Isi nama apotek dan buat akun pemilik (username, password, dan PIN otorisasi).
4. Selesai. Mulai sekarang aplikasi **berjalan tanpa internet** sampai masa aktif license habis.

Lokasi data aplikasi:

| Sistem | Folder |
|---|---|
| Windows | `%APPDATA%\GeoPOSTik\` (build `tauri dev`: `%APPDATA%\GeoPOSTik-dev\`) |
| macOS | `~/Library/Application Support/GeoPOSTik-dev/` |

Folder ini berisi `geopostik.db` (database), `license.json` (license), `logs/`, dan `backups/`.
File `license.json` terikat ke komputer ini dan ditandatangani, jadi tidak bisa diedit atau
disalin ke komputer lain.

## 6. Membuat installer Windows

Installer `.msi` / `.exe` **harus dibuat di Windows**, karena Tauri tidak mendukung
cross-compile installer dari macOS/Linux.

```bash
npm run tauri build
```

Hasilnya ada di:
- `src-tauri\target\release\bundle\msi\GeoPOSTik_0.1.0_x64_en-US.msi`
- `src-tauri\target\release\bundle\nsis\GeoPOSTik_0.1.0_x64-setup.exe`

Pasang installer di komputer apotek. Saat pertama dibuka, lakukan
[aktivasi & setup awal](#5-aktivasi--setup-awal) seperti di atas, dengan komputer terhubung
ke internet.

## 7. Memperpanjang atau mengganti license

Setelah masa aktif habis, semua menu kecuali **Pengaturan** terkunci dan menampilkan kotak
*"Masukkan license anda, dapatkan license di https://geolicense.my.id/"*.

1. Perpanjang license di [geolicense.my.id](https://geolicense.my.id/).
2. Hubungkan komputer apotek ke internet.
3. Klik **Validasi Ulang** di kotak license, atau buka **Pengaturan → License → Validasi Ulang**.

Untuk memakai license key lain, login sebagai pemilik, buka **Pengaturan → License**, lalu
isi **Ganti license key**. License lama tetap dipakai kalau license baru ditolak.

## 8. Masalah umum

| Masalah | Solusi |
|---|---|
| "Tidak dapat terhubung ke server license" | Pastikan internet aktif dan https://geolicense.my.id/ bisa dibuka, lalu coba lagi. |
| "Jumlah komputer untuk license ini sudah penuh" | Seat license habis. Pilih paket dengan seat lebih banyak di GeoLicense. |
| "Jam komputer lebih mundur dari pemakaian terakhir" | Perbaiki tanggal & jam Windows, lalu klik **Validasi Ulang**. |
| Aplikasi meminta aktivasi lagi setelah pindah komputer / install ulang Windows | Wajar, karena license terikat ke komputer. Aktifkan lagi dengan key yang sama (memakai seat baru). |
| "Tidak terhubung ke backend" | Halaman dibuka di browser biasa. Jalankan lewat `npm run tauri dev`. |

---

## Untuk pengembang

| Perintah | Fungsi |
|---|---|
| `npm run tauri dev` | Menjalankan aplikasi dalam mode pengembangan |
| `npm run tauri build` | Membuat installer (di Windows) |
| `npm run test:rust` | Menjalankan test backend Rust |
| `npm run bindings` | Membuat ulang tipe TypeScript dari struct Rust (`src/app/bindings/`) |

Dokumentasi:
- Alur aplikasi: [docs/FLOW.md](docs/FLOW.md)
- Skema database: [docs/SCHEMA.md](docs/SCHEMA.md)
- Arsitektur, struktur proyek & license: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
