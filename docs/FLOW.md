# Rancangan Alur (Flow) GeoPOSTik — POS Apotek

Dokumen ini adalah rancangan alur aplikasi sebelum masuk ke tahap coding.

Konteks: pengganti aplikasi POS lama milik klien yang sudah usang. **Dibangun dari nol,
tanpa migrasi data dari aplikasi lama** (klien sudah setuju). Data awal diisi lewat
import Excel dan stok opname awal.

## 0. Keputusan teknologi (final)

| Aspek | Pilihan |
|---|---|
| Platform | Windows 11, **satu PC kasir**, berjalan tanpa internet |
| Bentuk aplikasi | Aplikasi desktop (Tauri), **bukan** diakses lewat browser |
| Tampilan (UI) | Angular, dirender oleh WebView2 bawaan Windows |
| Backend | Rust (di dalam proses aplikasi yang sama, komunikasi lewat IPC `invoke()`, tanpa HTTP/port) |
| Database | SQLite (satu file). Tidak ada rencana multi-PC, jadi tidak perlu PostgreSQL |
| Installer | `.msi` / `.exe`, WebView2 offline installer ikut dibundel bila perlu |

Alternatif yang pernah dipertimbangkan: C# .NET + WPF + SQLite (paling "native" Windows),
Electron + Angular (terlalu berat), JavaFX, dan Spring Boot + Angular (terlalu berat untuk POS).

### Prinsip teknis

- **Uang disimpan sebagai integer (rupiah)**, tidak pernah float. Pembulatan harga jual
  bisa diatur (misal kelipatan Rp100 / Rp500).
- **Stok disimpan dalam satuan terkecil** (misal tablet). Box/strip hanyalah konversi,
  sehingga menjual 1 strip dari stok "box" tidak butuh proses buka kemasan.
- **Kartu stok bersifat append-only.** Angka stok di tabel batch hanyalah ringkasan
  yang selalu bisa dicocokkan ulang dari kartu stok.
- **Tidak ada hapus data transaksi.** Batal = ditandai batal + transaksi pembalik.
- **Log audit** untuk aksi sensitif: ubah harga, diskon di atas batas, void, retur,
  penyesuaian stok, ubah user/hak akses.
- **Penjagaan jam komputer:** aplikasi offline bergantung pada jam Windows (ED, laporan).
  Bila jam sistem lebih mundur dari transaksi terakhir, aplikasi memberi peringatan dan
  meminta otorisasi Pemilik.
- **Cetak struk langsung ESC/POS dari Rust** (bukan lewat dialog print WebView),
  termasuk perintah buka laci uang.
- **Backup saat aplikasi berjalan** memakai `VACUUM INTO`, bukan menyalin file `.db` mentah.
- **Kasir harus cepat dengan keyboard:** semua aksi utama punya shortcut (F1–F12),
  pencarian nama obat instan, barcode scanner sebagai input keyboard.

---

## 1. Pengguna dan hak akses

Empat peran. Satu user boleh punya **lebih dari satu peran** (misal pemilik yang juga
apoteker); haknya adalah gabungan dari semua perannya.

| Peran | Kode | Siapa |
|---|---|---|
| **Pemilik** | `OWNER` | Pemilik Sarana Apotek (PSA), sering bukan apoteker |
| **Apoteker** | `PHARMACIST` | APJ / apoteker pendamping, ber-SIPA |
| **TTK** | `TECHNICIAN` | Tenaga Teknis Kefarmasian, ber-SIPTTK |
| **Kasir** | `CASHIER` | Staf non-farmasi |

Matriks hak akses:

| Hak | Pemilik | Apoteker | TTK | Kasir |
|---|:-:|:-:|:-:|:-:|
| Penjualan bebas, buka/tutup shift, cetak ulang struk | ✓ | ✓ | ✓ | ✓ |
| Jual obat keras / OWA (otorisasi PIN) | – | ✓ | – | – |
| Resep: input | ✓ | ✓ | ✓ | – |
| Resep: skrining/validasi, narkotika & psikotropika | – | ✓ | – | – |
| Master obat (data obat) | ✓ | ✓ | ✓ | – |
| Ubah harga jual & margin | ✓ | *(atur)* | – | – |
| Penerimaan barang | ✓ | ✓ | ✓ | – |
| Hutang & pembayaran supplier | ✓ | – | – | – |
| Stok opname: input hitung | ✓ | ✓ | ✓ | – |
| Stok opname: setujui penyesuaian | ✓ | ✓ | – | – |
| Pemusnahan obat | – | ✓ | – | – |
| Void / retur (otorisasi PIN) | ✓ | ✓ | – | – |
| Diskon di atas batas (otorisasi PIN) | ✓ | ✓ | – | – |
| Lihat HPP, laba, nilai persediaan | ✓ | *(atur)* | – | – |
| Laporan penjualan | ✓ | ✓ | shift sendiri | shift sendiri |
| Laporan SIPNAP | ✓ | ✓ | – | – |
| User, pengaturan, backup/restore | ✓ | – | – | – |
| Log audit | ✓ | – | – | – |

- *(atur)* = diatur pemilik lewat pengaturan (default: apoteker boleh).
- **HPP dan laba tidak pernah tampil untuk TTK dan kasir**, termasuk di layar penerimaan
  barang milik TTK (harga beli tetap diinput, tetapi HPP hasil hitung dan margin disembunyikan).
- Pemilik sengaja tidak memegang kewenangan teknis kefarmasian (obat keras, narkotika,
  pemusnahan). Bila pemilik juga apoteker, beri dua peran.

Otorisasi (diskon besar, void, retur, obat keras) dilakukan di layar yang sama dengan
memasukkan PIN user yang punya **hak** tersebut, lalu dicatat di log audit (`authorized_by`).

Operasional: **satu shift per hari**. Layar terkunci otomatis setelah tidak aktif beberapa
menit (bisa diatur); membuka kunci cukup dengan PIN.

## 2. Gambaran alur besar

```
Setup awal ─► Master data ─► Stok opname awal ─► Pembelian / penerimaan barang ─► Stok (per batch + ED)
                                                                                   │
                ┌──────────────────────────────────────────────────────────────────┤
                ▼                  ▼                ▼                              ▼
          Penjualan bebas   Penjualan resep     Retur                Stok opname / pemusnahan
                └────────┬─────────┘
                         ▼
              Laporan ─► Backup harian
```

**Prinsip utama:** stok tidak pernah diubah langsung. Setiap perubahan stok selalu berasal
dari transaksi (stok awal, pembelian, penjualan, retur, opname, pemusnahan) dan tercatat di
**kartu stok**.

## 3. Detail tiap alur

### A. Setup awal (sekali saat pertama dipakai)
```
Install ─► Isi profil apotek (nama, alamat, SIA, apoteker penanggung jawab, SIPA)
       ─► Pengaturan pajak (PKP ya/tidak), pembulatan harga, margin default
       ─► Buat akun Pemilik ─► Atur printer struk ─► Atur lokasi backup
       ─► Import master dari Excel (template disediakan): obat + satuan, supplier, dokter
       ─► Stok opname awal (lihat A2)
```

### A2. Stok opname awal (pengganti migrasi data)
```
Cetak lembar hitung per rak ─► Hitung fisik per obat per batch (no batch + ED)
   ─► Input (atau import Excel) stok awal per batch + harga beli (HPP awal)
   ─► Review selisih/data kosong ─► Pemilik/Apoteker kunci stok awal
   ─► Kartu stok tercatat dengan jenis "STOK AWAL"
```
Setelah dikunci, stok hanya bisa berubah lewat transaksi biasa.

### B. Login dan shift kasir
```
Login ─► Kasir? ──ya──► Buka shift (input uang modal awal) ─► Menu kasir
                         ...
                         Tutup shift ─► Hitung uang fisik ─► Bandingkan dengan sistem
                                     ─► Selisih dicatat ─► Cetak laporan shift
```
Penjualan hanya bisa dilakukan bila ada shift yang terbuka.

### C. Master data obat
Data per obat:
- kode, nama dagang, nama generik, pabrik, kategori
- **barcode bisa lebih dari satu** (per satuan), obat tanpa barcode dicari lewat nama/kode
- **golongan**: bebas / bebas terbatas / keras / psikotropika / narkotika
- **flag OWA** (Obat Wajib Apotek): obat keras yang boleh diserahkan apoteker tanpa resep
- **satuan bertingkat**, misalnya 1 box = 10 strip, 1 strip = 10 tablet; stok disimpan
  dalam satuan terkecil
- **harga jual per satuan:** default otomatis dari margin (per kategori atau per obat),
  bisa diubah manual; peringatan bila harga jual di bawah HPP
- **harga grosir/bertingkat** per satuan jual, misal beli ≥ 5 strip harga lebih murah
- stok minimal (untuk peringatan) dan rak

Master pendukung di menu **Master Data**: kategori, rak, pabrik (dipilih lewat dropdown saat
tambah/ubah obat). Semua daftar master (termasuk Obat) dipaginasi di server (25/50/100 baris per
halaman) dengan pencarian nama/kode. Setiap data punya **kode yang dibuat otomatis oleh sistem** (tidak bisa diisi
atau diubah manusia) dan bisa dicetak sebagai label barcode (misal
ditempel di rak) dan di-scan untuk mencari atau memilihnya. Setiap baris punya tombol **Edit**,
**Nonaktifkan/Aktifkan**, dan **Hapus**. Master data **tidak pernah dihapus permanen**: hapus
berarti soft delete (disembunyikan, riwayat tetap tersimpan). Hapus hanya bisa bila data belum
dipakai obat mana pun; bila sudah dipakai, nonaktifkan saja (tidak muncul lagi sebagai pilihan,
obat lama tetap utuh). Menyusul: supplier, dokter, pasien/pelanggan. Jenis satuan dibuat langsung
dari form obat.

### D. Pembelian dan penerimaan barang
```
(opsional) Buat Surat Pesanan/PO ke supplier ─► Cetak SP
                       │
Barang datang ─► Input faktur (no faktur, tanggal, jatuh tempo, tunai/kredit)
             ─► Per item: jumlah, satuan, harga beli, diskon bertingkat, NO BATCH, TANGGAL ED
             ─► PPN faktur: harga termasuk / belum termasuk PPN ─► HPP dihitung per batch
             ─► Validasi: ED terlalu dekat? harga beli naik? ─► peringatan
             ─► Simpan ─► stok batch bertambah + kartu stok tercatat
             ─► Kredit? ─► masuk daftar hutang supplier ─► pembayaran hutang
             ─► (opsional) update harga jual otomatis berdasarkan margin
```

### E. Penjualan bebas (tanpa resep)
```
Scan barcode / cari nama ─► pilih satuan (tablet/strip/box) ─► jumlah
   ─► Sistem cek golongan:
        • bebas / bebas terbatas ─► lanjut
        • keras (termasuk OWA) ─► peringatan + otorisasi PIN apoteker
        • psikotropika / narkotika ─► TOLAK, arahkan ke penjualan resep
   ─► Stok cukup?
   ─► Ambil batch dengan ED terdekat (FEFO), lewati batch expired/terkunci.
      Satu baris bisa memakai beberapa batch (dicatat per batch).
   ─► Diskon (dibatasi, di atas batas perlu otorisasi) ─► Total (dibulatkan sesuai pengaturan)
   ─► Harga otomatis ikut tier grosir bila qty mencapai batas
   ─► Pembayaran: tunai / QRIS statis / debit (dicatat manual, tanpa internet),
      bisa split dalam satu nota (misal sebagian tunai + sisanya QRIS)
   ─► Simpan ─► stok berkurang ─► cetak struk ─► buka laci uang (bila tunai)
```

### F. Penjualan dengan resep
```
Input resep: nomor, tanggal, dokter, pasien (nama, umur, alamat)
   ─► Input item obat
        • obat biasa: seperti penjualan bebas
        • racikan (puyer/kapsul): satu item jual berisi beberapa komponen obat;
          stok tiap komponen berkurang; + biaya jasa racik & embalase
   ─► Apoteker validasi (skrining resep) ─► cetak etiket/label aturan pakai
   ─► Bayar di kasir ─► struk + copy resep bila perlu
   ─► Narkotika/psikotropika tercatat khusus untuk laporan bulanan (format SIPNAP)
```

### G. Retur
- **Retur penjualan:** cari nomor struk ─► pilih item ─► alasan ─► otorisasi PIN pemilik/apoteker
  ─► stok kembali ke batch asal ─► uang dikembalikan.
- **Retur ke supplier:** pilih faktur/batch (rusak, mendekati ED) ─► stok berkurang
  ─► potong hutang atau tunggu barang pengganti.

### H. Stok opname dan obat kedaluwarsa
```
Mulai opname (per rak/kategori) ─► cetak lembar hitung ─► input stok fisik per batch
   ─► Sistem hitung selisih ─► Pemilik/Apoteker setujui ─► penyesuaian stok + kartu stok

Dashboard harian: obat ED ≤ 3 bulan, stok di bawah minimal
   ─► Expired ─► batch terkunci otomatis (tidak bisa dijual)
   ─► Pemusnahan: pilih batch ─► berita acara ─► stok nol
```

### I. Batal (void) transaksi
Hanya dengan **otorisasi PIN Pemilik/Apoteker** dan wajib diberi alasan. Data tidak dihapus,
hanya ditandai batal, dan stok dikembalikan ke batch asal lewat kartu stok.

### J. Laporan
- Penjualan harian/bulanan (per kasir, per shift, per metode bayar)
- Laba kotor (harga jual dikurangi HPP per batch)
- Obat terlaris dan obat tidak laku (slow moving)
- Kartu stok per obat, nilai persediaan
- Pembelian dan hutang jatuh tempo
- Obat hampir ED dan sudah ED
- Laporan narkotika/psikotropika (format SIPNAP)
- Log audit

### K. Backup dan restore
Backup otomatis (`VACUUM INTO`) setiap tutup shift dan tutup aplikasi, menyimpan beberapa
salinan terakhir, dan ada tombol backup manual ke flashdisk. Restore hanya bisa dilakukan Pemilik.

## 4. Daftar layar

1. Login
2. Dashboard (peringatan ED, stok menipis, omzet hari ini)
3. Kasir/POS
4. Penjualan resep
5. Obat (daftar & form obat, harga, tier grosir)
6. Master Data: kategori, rak, pabrik (menyusul supplier, dokter, pasien)
7. Pembelian/penerimaan barang, hutang supplier
8. Retur (penjualan dan supplier)
9. Stok: stok awal, kartu stok, stok opname, pemusnahan
10. Laporan
11. Pengguna (tabel seperti Obat: cari, filter peran, aksi massal; soft delete; semua perubahan masuk log)
12. Pengaturan: profil apotek, pajak & harga, printer, backup

## 5. Data utama (gambaran, belum tabel)

Obat, satuan obat, barcode, **batch obat (stok ada di sini)**, supplier, pembelian + detail,
penjualan + detail + **detail per batch yang terpakai**, resep + racikan (komponen), dokter,
pasien, retur, stok opname, **kartu stok (append-only)**, shift kasir, user, **log audit**,
pengaturan.

## 6. Keputusan yang sudah diambil

| Pertanyaan | Keputusan |
|---|---|
| Jumlah PC kasir | Satu PC → SQLite |
| Data aplikasi lama | Tidak dimigrasi; mulai dari nol lewat import Excel + stok opname awal |
| Resep dan racikan | Tahap 3, tetapi struktur data dirancang dari awal |
| Pajak | Ada pengaturan PKP ya/tidak; PPN pembelian selalu ditangani |
| Member/pelanggan | Ditunda; cukup data pelanggan dulu |
| Harga jual | Default dari margin, bisa diubah manual, peringatan bila di bawah HPP |

## 7. Tahapan

| Tahap | Isi |
|---|---|
| **MVP** | Login + hak akses, pengaturan dasar, master obat + satuan + barcode, import Excel, stok opname awal, penerimaan barang (batch + ED + PPN), shift kasir, kasir bebas dengan FEFO + otorisasi obat keras, struk ESC/POS, void, kartu stok, dashboard ED/stok minimal, laporan penjualan harian & per shift, log audit, backup |
| **Tahap 2** | Retur penjualan & supplier, stok opname berkala, pemusnahan, hutang supplier, laporan laba & nilai persediaan |
| **Tahap 3** | Resep dan racikan, etiket, laporan narkotika/psikotropika (SIPNAP), PO ke supplier, slow moving |
