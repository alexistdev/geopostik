# Rancangan Alur (Flow) GeoPOSTik — POS Apotek

Dokumen ini adalah rancangan alur aplikasi sebelum masuk ke tahap coding.

## 0. Keputusan teknologi (sementara)

| Aspek | Pilihan |
|---|---|
| Platform | Windows 11, berjalan tanpa internet |
| Bentuk aplikasi | Aplikasi desktop (Tauri), **bukan** diakses lewat browser |
| Tampilan (UI) | Angular, dirender oleh WebView2 bawaan Windows |
| Backend | Rust (di dalam proses aplikasi yang sama, komunikasi lewat IPC `invoke()`, tanpa HTTP/port) |
| Database | SQLite (satu file). Ganti ke PostgreSQL bila kasir lebih dari satu PC dalam LAN |
| Installer | `.msi` / `.exe`, WebView2 offline installer ikut dibundel bila perlu |

Alternatif yang pernah dipertimbangkan: C# .NET + WPF + SQLite (paling "native" Windows),
Electron + Angular (terlalu berat), JavaFX, dan Spring Boot + Angular (terlalu berat untuk POS).

---

## 1. Pengguna dan hak akses

| Peran | Yang boleh dilakukan |
|---|---|
| **Pemilik/Admin** | Semua menu: pengaturan, user, harga, laporan keuangan, backup |
| **Apoteker** | Master obat, pembelian, validasi resep, stok opname, pemusnahan obat |
| **Kasir** | Penjualan, buka/tutup shift, cetak ulang struk |

## 2. Gambaran alur besar

```
Setup awal ─► Master data ─► Pembelian / penerimaan barang ─► Stok (per batch + ED)
                                                               │
                ┌──────────────────────────────────────────────┤
                ▼                  ▼                ▼          ▼
          Penjualan bebas   Penjualan resep     Retur    Stok opname / pemusnahan
                └────────┬─────────┘
                         ▼
              Laporan ─► Backup harian
```

**Prinsip utama:** stok tidak pernah diubah langsung. Setiap perubahan stok selalu berasal
dari transaksi (pembelian, penjualan, retur, opname, pemusnahan) dan tercatat di **kartu stok**.

## 3. Detail tiap alur

### A. Setup awal (sekali saat pertama dipakai)
```
Install ─► Isi profil apotek (nama, alamat, SIA, apoteker penanggung jawab, SIPA)
       ─► Buat akun Admin ─► Atur printer struk ─► Atur lokasi backup
       ─► Import data obat awal (Excel) + stok awal per batch
```

### B. Login dan shift kasir
```
Login ─► Kasir? ──ya──► Buka shift (input uang modal awal) ─► Menu kasir
                         ...
                         Tutup shift ─► Hitung uang fisik ─► Bandingkan dengan sistem
                                     ─► Selisih dicatat ─► Cetak laporan shift
```

### C. Master data obat
Data per obat:
- kode, barcode, nama dagang, nama generik, pabrik, kategori
- **golongan**: bebas / bebas terbatas / keras / psikotropika / narkotika → menentukan perlu resep atau tidak
- **satuan bertingkat**, misalnya 1 box = 10 strip, 1 strip = 10 tablet, dengan harga jual per satuan
- stok minimal (untuk peringatan) dan lokasi rak

Master pendukung: supplier, dokter, pasien/pelanggan, kategori, satuan.

### D. Pembelian dan penerimaan barang
```
(opsional) Buat Surat Pesanan/PO ke supplier ─► Cetak SP
                       │
Barang datang ─► Input faktur (no faktur, tanggal, jatuh tempo, tunai/kredit)
             ─► Per item: jumlah, satuan, harga beli, diskon, NO BATCH, TANGGAL ED
             ─► Validasi: ED terlalu dekat? harga beli naik? ─► peringatan
             ─► Simpan ─► stok batch bertambah + kartu stok tercatat
             ─► Kredit? ─► masuk daftar hutang supplier ─► pembayaran hutang
             ─► (opsional) update harga jual otomatis berdasarkan margin
```

### E. Penjualan bebas (tanpa resep)
```
Scan barcode / cari nama ─► pilih satuan (tablet/strip/box) ─► jumlah
   ─► Sistem cek:
        • obat keras/narkotika? ─► TOLAK, arahkan ke penjualan resep
        • stok cukup?
        • ambil batch dengan ED terdekat (FEFO), lewati batch yang sudah expired
   ─► Diskon (dibatasi, di atas batas perlu otorisasi) ─► Total
   ─► Pembayaran: tunai / QRIS statis / debit (dicatat manual, tanpa internet)
   ─► Simpan ─► stok berkurang ─► cetak struk
```

### F. Penjualan dengan resep
```
Input resep: nomor, tanggal, dokter, pasien (nama, umur, alamat)
   ─► Input item obat (termasuk racikan: puyer/kapsul + biaya jasa racik & embalase)
   ─► Apoteker validasi (skrining resep) ─► cetak etiket/label aturan pakai
   ─► Bayar di kasir ─► struk + copy resep bila perlu
   ─► Narkotika/psikotropika tercatat khusus untuk laporan bulanan
```

### G. Retur
- **Retur penjualan:** cari nomor struk ─► pilih item ─► alasan ─► otorisasi apoteker/admin
  ─► stok kembali ke batch asal ─► uang dikembalikan.
- **Retur ke supplier:** pilih faktur/batch (rusak, mendekati ED) ─► stok berkurang
  ─► potong hutang atau tunggu barang pengganti.

### H. Stok opname dan obat kedaluwarsa
```
Mulai opname (per rak/kategori) ─► cetak lembar hitung ─► input stok fisik per batch
   ─► Sistem hitung selisih ─► Admin setujui ─► penyesuaian stok + kartu stok

Dashboard harian: obat ED ≤ 3 bulan, stok di bawah minimal
   ─► Expired ─► batch terkunci otomatis (tidak bisa dijual)
   ─► Pemusnahan: pilih batch ─► berita acara ─► stok nol
```

### I. Batal (void) transaksi
Hanya dengan **otorisasi Admin/Apoteker** dan wajib diberi alasan. Data tidak dihapus,
hanya ditandai batal, dan stok dikembalikan.

### J. Laporan
- Penjualan harian/bulanan (per kasir, per shift, per metode bayar)
- Laba kotor (harga jual dikurangi harga beli per batch)
- Obat terlaris dan obat tidak laku (slow moving)
- Kartu stok per obat, nilai persediaan
- Pembelian dan hutang jatuh tempo
- Obat hampir ED dan sudah ED
- Laporan narkotika/psikotropika

### K. Backup dan restore
Backup otomatis setiap tutup aplikasi atau tutup shift terakhir, menyimpan beberapa salinan
terakhir, dan ada tombol backup manual ke flashdisk. Restore hanya bisa dilakukan Admin.

## 4. Daftar layar

1. Login
2. Dashboard (peringatan ED, stok menipis, omzet hari ini)
3. Kasir/POS
4. Penjualan resep
5. Master: obat, supplier, dokter, pasien, user
6. Pembelian/penerimaan barang, hutang supplier
7. Retur (penjualan dan supplier)
8. Stok: kartu stok, stok opname, pemusnahan
9. Laporan
10. Pengaturan: profil apotek, printer, backup

## 5. Data utama (gambaran, belum tabel)

Obat, satuan obat, **batch obat (stok ada di sini)**, supplier, pembelian + detail,
penjualan + detail (termasuk batch yang terpakai), resep, dokter, pasien, retur,
stok opname, **kartu stok**, shift kasir, user, pengaturan.

## 6. Pertanyaan yang belum diputuskan

1. **Jumlah PC kasir:** satu PC saja, atau beberapa PC dalam LAN? (SQLite vs PostgreSQL)
2. **Resep dan racikan:** perlu dari versi pertama, atau penjualan bebas dulu?
3. **Pajak:** apotek sudah PKP (ada PPN) atau belum?
4. **Member/pelanggan:** perlu fitur poin atau harga khusus?
5. **Harga jual:** manual, atau otomatis dari margin di atas harga beli?

## 7. Usulan tahapan

| Tahap | Isi |
|---|---|
| **MVP** | Login, master obat + satuan, penerimaan barang (batch + ED), kasir bebas dengan FEFO, struk, kartu stok, laporan penjualan harian, backup |
| **Tahap 2** | Shift kasir, retur, stok opname, dashboard ED/stok minimal, hutang supplier |
| **Tahap 3** | Resep dan racikan, laporan narkotika/psikotropika, laba, PO ke supplier |
