# Rancangan Skema Database GeoPOSTik

Turunan dari [FLOW.md](FLOW.md). Masih tahap rancangan, belum migration/DDL final.

## 0. Konvensi

| Aspek | Aturan |
|---|---|
| Nama tabel/kolom | Bahasa Inggris, `snake_case`, tabel jamak (`products`, `sale_items`) |
| Primary key | `id INTEGER PRIMARY KEY` (rowid SQLite) |
| Uang | `INTEGER` rupiah. Tidak ada `REAL` untuk uang |
| HPP per satuan terkecil | `INTEGER` rupiah × 100 (kolom berakhiran `_x100`), karena HPP per tablet bisa pecahan (Rp15.050 / 100 tablet = Rp150,50) |
| Persentase | `INTEGER` basis point (`_bp`): 10% = 1000, 12,5% = 1250 |
| Jumlah stok | `INTEGER` dalam **satuan terkecil** (kolom berakhiran `_base`) |
| Tanggal & waktu | `TEXT` waktu lokal `YYYY-MM-DD HH:MM:SS`; tanggal saja `YYYY-MM-DD` (ED, tanggal faktur) |
| Boolean | `INTEGER` 0/1 |
| Enum | `TEXT` + `CHECK (kolom IN (...))` |
| Master data (termasuk barcode obat) | **Tidak pernah dihapus permanen.** Nonaktif = `is_active = 0` (tidak muncul sebagai pilihan). Hapus = soft delete `deleted_at` + `deleted_by` (disembunyikan dari daftar). Trigger menolak `DELETE` di semua tabel master |
| Transaksi | Tidak dihapus, ditandai `VOID` + transaksi pembalik |
| PRAGMA | `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL` |

Semua tabel punya `created_at`; tabel yang bisa diubah punya `updated_at`. Kolom ini tidak
diulang di daftar di bawah agar ringkas.

---

## 1. Pengguna, pengaturan, audit

### `users`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| username | TEXT UNIQUE | |
| full_name | TEXT | |
| password_hash | TEXT | argon2 |
| pin_hash | TEXT NULL | PIN untuk otorisasi dan buka kunci layar |
| license_type | TEXT NULL | `SIPA` / `SIPTTK` |
| license_number | TEXT NULL | dicetak di etiket, copy resep, laporan SIPNAP |
| is_active | INTEGER | |
| deleted_at, deleted_by | TEXT, FK users NULL | soft delete dari menu Pengguna |
| created_by | FK users NULL | pemilik yang membuat akun; NULL untuk pemilik dari setup awal |

Menu **Pengguna** (hak `USER_MANAGE`, migration `011_user_soft_delete.sql`): user **tidak pernah
dihapus permanen** (trigger menolak `DELETE`). Hapus = soft delete: disembunyikan dari daftar dan
tidak bisa login lagi, tetapi transaksi dan log yang menunjuk ke user itu tetap utuh. Username user
terhapus tetap dicadangkan (tidak bisa dipakai lagi) agar log lama tidak tertukar orang. Aturan:
akun sendiri tidak bisa dinonaktifkan/dihapus atau dilepas peran Pemiliknya, dan harus selalu ada
minimal satu Pemilik aktif. Nomor SIPA wajib untuk Apoteker, SIPTTK opsional untuk TTK. Setiap
perubahan (tambah, ubah data/peran, aktif/nonaktif, hapus) dicatat di `audit_logs` entity `users`
sebagai snapshot sebelum/sesudah; ganti password/PIN dicatat sebagai `PASSWORD_CHANGE` /
`PIN_CHANGE` tanpa nilainya.

### `user_roles`
| Kolom | Tipe | Keterangan |
|---|---|---|
| user_id | FK users | |
| role | TEXT | `OWNER` / `PHARMACIST` / `TECHNICIAN` / `CASHIER` |
| PK | (user_id, role) | satu user bisa punya beberapa peran |

Daftar hak (permission) dan pemetaan peran → hak didefinisikan **di kode Rust** (enum),
bukan di database. Dua hak apoteker yang bisa diatur pemilik disimpan di `settings`:
`access.pharmacist_can_view_cost` dan `access.pharmacist_can_edit_price`.

### `settings`
| Kolom | Tipe | Keterangan |
|---|---|---|
| key | TEXT PK | misal `pharmacy.profile`, `tax.is_pkp`, `price.rounding`, `price.default_margin_bp`, `printer.receipt`, `backup`, `access.*`, `session.lock_after_minutes` |
| value | TEXT | JSON |

### `audit_logs`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| user_id | FK users | yang melakukan aksi |
| authorized_by | FK users NULL | yang memasukkan PIN otorisasi |
| action | TEXT | misal `PRICE_CHANGE`, `SALE_VOID`, `DISCOUNT_OVERRIDE`, `HARD_DRUG_SALE`, `CLOCK_BACKWARD` |
| entity, entity_id | TEXT, INTEGER | objek yang terdampak |
| detail | TEXT | JSON sebelum/sesudah |
| reason | TEXT NULL | |

Append-only: trigger menolak `UPDATE`/`DELETE`.

### `doc_sequences`
Penomoran dokumen per hari/bulan, misal `PJ-260925-0001`.

| Kolom | Tipe | Keterangan |
|---|---|---|
| prefix | TEXT | `PJ` (penjualan), `PB` (pembelian), `OP` (opname), dst. |
| period | TEXT | `260925` atau `2609` |
| last_no | INTEGER | |
| PK | (prefix, period) | |

---

## 2. Master data

### `categories`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| code | TEXT UNIQUE | kode/label barcode, dibuat sistem `KTG0001`, tidak bisa diubah |
| name | TEXT UNIQUE | |
| margin_bp | INTEGER NULL | margin default kategori; NULL = pakai margin global |
| created_at, created_by | TEXT, FK users NULL | kapan & oleh siapa dibuat (tampil di Master Data) |

### `units`
Kamus satuan: tablet, kapsul, strip, box, botol, tube, pcs, dst.

| Kolom | Tipe |
|---|---|
| id | INTEGER PK |
| name | TEXT UNIQUE |

### `products`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| code | TEXT UNIQUE | kode internal |
| name | TEXT | nama dagang |
| generic_name | TEXT NULL | |
| manufacturer_id | FK manufacturers NULL | pabrik |
| category_id | FK categories | |
| drug_class | TEXT | `FREE` / `LIMITED_FREE` / `HARD` / `PSYCHOTROPIC` / `NARCOTIC` |
| is_owa | INTEGER | Obat Wajib Apotek |
| base_unit_id | FK units | satuan terkecil, satuan penyimpanan stok |
| min_stock_base | INTEGER | batas peringatan stok |
| rack_id | FK racks NULL | lokasi rak |
| margin_bp | INTEGER NULL | override margin; NULL = ikut kategori |
| last_cost_x100 | INTEGER NULL | HPP per satuan terkecil dari pembelian terakhir (dasar harga otomatis) |
| is_active | INTEGER | |

### `product_units` — satuan bertingkat + harga jual
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| product_id | FK products | |
| unit_id | FK units | |
| conversion | INTEGER | isi dalam satuan terkecil (tablet = 1, strip = 10, box = 100) |
| sell_price | INTEGER | harga jual per satuan ini |
| price_mode | TEXT | `AUTO` (dihitung dari HPP + margin + pembulatan) / `MANUAL` |
| is_default_sale | INTEGER | satuan yang terpilih otomatis saat scan nama |
| is_active | INTEGER | |
| UNIQUE | (product_id, unit_id) | |

Aturan: satu produk wajib punya baris dengan `conversion = 1` untuk `base_unit_id`.

### `price_tiers` — harga grosir / bertingkat per jumlah
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| product_unit_id | FK product_units | tier berlaku per satuan jual |
| min_qty | INTEGER | berlaku bila qty (dalam satuan ini) ≥ min_qty; wajib > 1 |
| price_mode | TEXT | `AUTO` / `MANUAL`, sama seperti `product_units` |
| margin_bp | INTEGER NULL | untuk `AUTO`: margin tier (lebih kecil dari margin eceran) |
| price | INTEGER | harga per satuan pada tier ini (untuk `AUTO` dihitung ulang saat HPP berubah) |
| is_active | INTEGER | |
| UNIQUE | (product_unit_id, min_qty) | |

Contoh: Paracetamol strip eceran Rp5.000; beli ≥ 5 strip Rp4.500; beli ≥ 20 strip Rp4.200.

Aturan:
- Harga yang dipakai = tier dengan `min_qty` terbesar yang ≤ qty baris; bila tidak ada, `product_units.sell_price`.
- Tier dihitung per baris. Scan ulang obat + satuan yang sama menambah qty baris yang ada
  (tidak membuat baris baru), sehingga tier ikut terhitung ulang.
- Validasi: harga tier harus menurun seiring naiknya `min_qty` dan tidak boleh di bawah HPP
  (peringatan di master + daftar di dashboard bila HPP naik melewati harga tier `MANUAL`).

### `product_barcodes`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| product_unit_id | FK product_units | scan barcode langsung tahu obat **dan** satuannya |
| barcode | TEXT | unik di antara barcode yang belum dihapus |
| deleted_at, deleted_by | TEXT, FK users NULL | soft delete saat barcode dilepas dari obat |

Barcode tidak pernah dihapus permanen (migration `006_barcode_soft_delete.sql`, trigger menolak
`DELETE`). Saat form obat disimpan hanya selisihnya yang diproses: barcode yang dilepas
di-soft-delete, barcode baru ditambahkan, barcode yang tidak berubah tetap baris yang sama.
Barcode yang sudah dilepas boleh dipakai obat lain.

### `racks`, `manufacturers`
Master sederhana (id, code, name UNIQUE tanpa beda huruf besar/kecil, is_active), dikelola di menu
**Master Data** dan dipilih lewat dropdown saat tambah/ubah obat. Ditambahkan di migration
`002_racks_manufacturers.sql`, yang juga memindahkan isi kolom teks lama `products.manufacturer`
dan `products.rack_location`.

**Soft delete** (migration `004_master_soft_delete.sql`): `categories`, `racks`, `manufacturers`
punya `deleted_at` dan `deleted_by`. Nama dan kode unik hanya di antara data yang belum dihapus
(index unik parsial), sehingga boleh dipakai lagi. Hapus ditolak bila data masih dipakai obat.
Master baru (supplier, dokter, pelanggan, dst.) mengikuti pola yang sama.

**Kode master** (`categories.code`, `racks.code`, `manufacturers.code`, migration `003`/`005`):
**selalu dibuat otomatis oleh sistem** (`KTG0001` / `RAK0001` / `PBR0001`), tidak pernah diisi
manusia, dan **tidak bisa diubah** setelah tersimpan (trigger menolak `UPDATE` kode). Kode
otomatis tidak memakai ulang kode milik data yang sudah dihapus. Dicetak sebagai label barcode
Code 128 dan bisa di-scan untuk mencari atau memilih data (misal scan label rak saat mengisi
form obat).

**Pembuat** (migration `007_master_created_by.sql`): `categories`, `racks`, `manufacturers` punya
`created_by` (FK users). Tabel Master Data menampilkan tanggal dibuat (`created_at`) dan username
pembuatnya. Data yang dibuat sebelum migration ini `created_by` = NULL (tampil "-").

### `suppliers`, `doctors`, `customers`
| Tabel | Kolom utama |
|---|---|
| suppliers | name, address, phone, npwp, payment_term_days, is_active |
| doctors | code, name, sip_number, specialty, address, phone, is_active |
| customers | code, name, gender (`M`/`F`), birth_date, address, phone, is_active (dipakai juga sebagai pasien resep) |

`doctors` dan `customers` (migration `013_prescriptions.sql`) mengikuti pola master lain: kode
otomatis `DOK0001` / `PSN0001` yang tidak bisa diubah, soft delete (`deleted_at`, `deleted_by`),
dan `created_by`. Hapus ditolak bila sudah dipakai resep.

---

## 3. Stok

### `batches` — tempat stok berada
Satu baris = satu **lot penerimaan** (satu baris faktur atau satu baris stok awal).
Nomor batch yang sama dari dua faktur berbeda menjadi dua baris, karena HPP-nya bisa
berbeda. Di layar tetap bisa ditampilkan dikelompokkan per nomor batch.

| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| product_id | FK products | |
| batch_number | TEXT | |
| expiry_date | TEXT | `YYYY-MM-DD` |
| unit_cost_x100 | INTEGER | HPP per satuan terkecil |
| qty_on_hand_base | INTEGER | **ringkasan**, hanya diubah oleh trigger kartu stok. `CHECK (qty_on_hand_base >= 0)` |
| is_locked | INTEGER | kunci manual (recall, rusak, menunggu pemusnahan) |
| lock_reason | TEXT NULL | alasan kunci, wajib saat mengunci (migration `012_stock.sql`) |
| source_type, source_id | TEXT, INTEGER | `OPENING` / `PURCHASE` / `ADJUSTMENT` (batch baru saat opname) + id baris asal |

Batch expired **tidak** perlu di-update statusnya; kondisi "bisa dijual" dihitung:
`qty_on_hand_base > 0 AND is_locked = 0 AND expiry_date > date('now','localtime')`.

Query FEFO:
```sql
SELECT id, qty_on_hand_base, unit_cost_x100
FROM batches
WHERE product_id = ? AND qty_on_hand_base > 0 AND is_locked = 0
  AND expiry_date > date('now','localtime')
ORDER BY expiry_date, id;
```

### `stock_movements` — kartu stok (append-only)
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | urutan kejadian |
| batch_id | FK batches | |
| product_id | FK products | denormalisasi untuk query kartu stok per obat |
| movement_type | TEXT | `OPENING`, `PURCHASE`, `PURCHASE_VOID`, `SALE`, `SALE_VOID`, `SALE_RETURN`, `SUPPLIER_RETURN`, `ADJUSTMENT`, `DESTRUCTION` |
| qty_change_base | INTEGER | bertanda: + masuk, − keluar; tidak boleh 0 |
| ref_type, ref_id | TEXT, INTEGER | dokumen asal (`sale`, `purchase`, `stock_opname`, ...) |
| ref_line_id | INTEGER NULL | baris dokumen asal |
| user_id | FK users | |
| note | TEXT NULL | |
| created_at | TEXT | |

Trigger:
1. `AFTER INSERT` → `UPDATE batches SET qty_on_hand_base = qty_on_hand_base + NEW.qty_change_base`.
   Karena ada `CHECK >= 0`, stok minus otomatis ditolak dan seluruh transaksi di-rollback.
2. `BEFORE UPDATE` / `BEFORE DELETE` → `RAISE(ABORT, 'stock_movements is append-only')`.

Saldo berjalan di kartu stok dihitung saat query dengan window function
`SUM(qty_change_base) OVER (PARTITION BY product_id ORDER BY id)`, jadi tidak disimpan.
Ada juga fitur "cek konsistensi" yang membandingkan `SUM` kartu stok dengan `qty_on_hand_base`.

### `stock_opnames` + `stock_opname_items`
Dipakai untuk **stok awal** (`OPENING`) sekaligus opname berkala (`PERIODIC`).

`stock_opnames`: id, number, opname_type (`OPENING`/`PERIODIC`), scope_note (rak/kategori),
status (`DRAFT`/`SUBMITTED`/`APPROVED`/`CANCELLED`), created_by, approved_by, approved_at.

`stock_opname_items`:
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| opname_id | FK | |
| product_id | FK | |
| batch_id | FK NULL | NULL = batch baru ditemukan (atau stok awal) |
| batch_number, expiry_date | TEXT | diisi bila batch_id NULL |
| unit_cost_x100 | INTEGER NULL | wajib untuk stok awal / batch baru |
| system_qty_base | INTEGER | snapshot saat opname dimulai |
| physical_qty_base | INTEGER NULL | hasil hitung |
| note | TEXT NULL | |

Saat `APPROVED`: batch baru dibuat, lalu satu `stock_movements` per selisih
(`OPENING` atau `ADJUSTMENT`).

Aturan (migration `012_stock.sql`, modul `src-tauri/src/inventory/`):
- Nomor `OP-YYMM-0001` dari `doc_sequences` (prefix `OP`, period bulanan).
- Satu batch hanya sekali per opname (index unik parsial `(opname_id, batch_id)`).
- Baris hanya bisa ditambah/diubah/dihapus selama opname `DRAFT` (trigger); pengecualian satu-satunya
  adalah pengisian `batch_id` batch baru saat approve. Opname tidak bisa dihapus (batal = `CANCELLED`).
- `OPENING` hanya berisi batch baru; `batch_id` baris diisi batch yang dibuat saat approve.
  Batch baru dengan fisik 0 tidak dibuat.
- Selisih = `physical_qty_base − system_qty_base` (snapshot saat baris ditambahkan).
- Pengaturan `stock.opening_locked` = `true` setelah **Kunci stok awal**: opname `OPENING` baru ditolak.
- HPP (`unit_cost_x100`) dan nilai selisih tidak dikirim ke user tanpa `VIEW_COST`; user tersebut tetap
  bisa mengisi HPP batch baru (kosong saat mengubah = pertahankan nilai lama).

### `destructions` + `destruction_items` (Tahap 2)
Header: number, destruction_date, witnesses, note, status (`DRAFT`/`APPROVED`/`CANCELLED`), created_by, approved_by.
Item: batch_id, qty_base → movement `DESTRUCTION`.

---

## 4. Pembelian

### `purchases`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| number | TEXT UNIQUE | nomor internal |
| supplier_id | FK | |
| invoice_number | TEXT | nomor faktur PBF |
| invoice_date, received_date, due_date | TEXT | |
| payment_type | TEXT | `CASH` / `CREDIT` |
| tax_mode | TEXT | `INCLUDED` / `EXCLUDED` / `NONE` |
| tax_rate_bp | INTEGER | snapshot tarif PPN |
| subtotal, discount_total, tax_total, grand_total | INTEGER | |
| status | TEXT | `DRAFT` / `POSTED` / `VOID` |
| created_by, posted_at | | |
| UNIQUE | (supplier_id, invoice_number) | mencegah faktur diinput dua kali |

Stok baru bertambah saat `POSTED`. Draft boleh disimpan setengah jadi.

### `purchase_items`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| purchase_id | FK | |
| product_id, product_unit_id | FK | satuan beli |
| qty | INTEGER | dalam satuan beli |
| conversion | INTEGER | snapshot isi satuan |
| unit_price | INTEGER | harga beli per satuan beli (sesuai faktur) |
| discount1_bp, discount2_bp | INTEGER | diskon bertingkat |
| line_total | INTEGER | setelah diskon, sebelum PPN |
| batch_number, expiry_date | TEXT | |
| unit_cost_x100 | INTEGER | HPP per satuan terkecil hasil hitung |
| batch_id | FK NULL | diisi saat posting |

**Aturan HPP:** apotek **non-PKP** → PPN masuk HPP (karena tidak bisa dikreditkan).
Apotek **PKP** → HPP tanpa PPN. Diskon faktur tingkat header dialokasikan proporsional ke baris.

### `supplier_payments`
number (`BH-YYMM-0001`), supplier_id, purchase_id, payment_date, amount, method (`CASH`/`TRANSFER`/`GIRO`),
reference, note, created_by, voided_at, voided_by, void_reason.
Sisa hutang = `grand_total − SUM(payments yang tidak dibatalkan) − SUM(retur yang memotong hutang)`
(`purchasing::OUTSTANDING`, dipakai juga Dashboard).

### Aturan tambahan (migration `015_purchasing.sql`, modul `src-tauri/src/purchasing/`)
- `suppliers` mengikuti pola master: `code` otomatis `SUP0001` (tidak bisa diubah), soft delete
  (`deleted_at`, `deleted_by`), `created_by`, nama unik di antara yang belum dihapus.
- `purchases` dibangun ulang: `UNIQUE (supplier_id, invoice_number)` diganti index unik parsial
  `WHERE status <> 'VOID'`; kolom baru `extra_discount` (diskon faktur, rupiah), `tax_in_cost` (snapshot
  PKP saat posting), `posted_by`, `voided_at`, `voided_by`, `void_reason`. Nomor `PB-YYMM-0001`.
  `subtotal` = Σ qty × harga; `discount_total` = diskon baris + diskon faktur.
- `purchase_items` baru: `line_no`, `bonus_qty` (satuan beli, tanpa harga). Stok masuk
  `(qty + bonus_qty) × conversion`; `unit_cost_x100` dan `batch_id` diisi saat posting.
- Trigger: faktur tidak bisa dihapus; POSTED hanya boleh menjadi VOID tanpa mengubah angka; VOID final;
  baris faktur hanya bisa diubah selama DRAFT. Pembayaran tidak bisa dihapus/diubah selain dibatalkan,
  dan hanya untuk faktur `POSTED` + `CREDIT`.
- Posting: satu batch per baris (`source_type = 'PURCHASE'`, `source_id` = baris faktur), movement
  `PURCHASE` (`ref_type = 'purchase'`). Batal faktur diposting: movement `PURCHASE_VOID` per batch, hanya
  bila batch belum punya movement lain dan faktur belum punya pembayaran aktif.
- Pengaturan `tax.is_pkp` (default `false`) dan `tax.ppn_rate_bp` (default 1100).

---

## 5. Shift dan penjualan

### `shifts`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| user_id | FK | kasir |
| opened_at, closed_at | TEXT | |
| opening_cash | INTEGER | modal awal |
| expected_cash | INTEGER NULL | snapshot saat tutup: modal + tunai masuk − kembalian − refund |
| counted_cash | INTEGER NULL | hitungan fisik |
| difference | INTEGER NULL | |
| status | TEXT | `OPEN` / `CLOSED` |

Hanya satu shift terbuka: `CREATE UNIQUE INDEX one_open_shift ON shifts(status) WHERE status = 'OPEN';`

### `sales`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| number | TEXT UNIQUE | nomor struk |
| shift_id | FK | |
| cashier_id | FK users | |
| customer_id | FK NULL | |
| prescription_id | FK NULL | diisi untuk penjualan resep |
| sale_type | TEXT | `OTC` / `PRESCRIPTION` |
| sold_at | TEXT | |
| subtotal, discount_total, tax_total, rounding, grand_total | INTEGER | |
| status | TEXT | `COMPLETED` / `VOID` |
| voided_at, voided_by, void_reason | NULL | |

### `sale_items`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| sale_id | FK | |
| line_no | INTEGER | |
| item_kind | TEXT | `PRODUCT` / `COMPOUND` (racikan) / `SERVICE` (jasa racik, embalase) |
| parent_item_id | FK sale_items NULL | komponen racikan menunjuk ke baris `COMPOUND` |
| product_id, product_unit_id | FK NULL | NULL untuk `COMPOUND`/`SERVICE` |
| description | TEXT | nama yang dicetak di struk (snapshot) |
| qty | INTEGER | dalam satuan jual |
| conversion | INTEGER | snapshot |
| qty_base | INTEGER | qty × conversion |
| unit_price | INTEGER | snapshot harga (sudah harga tier bila berlaku) |
| price_tier_id | FK price_tiers NULL | tier yang dipakai; NULL = harga eceran |
| discount_amount | INTEGER | |
| line_total | INTEGER | untuk komponen racikan = 0 di struk; nilainya dijumlahkan ke baris induk |
| usage_instruction | TEXT NULL | aturan pakai untuk etiket (Tahap 3) |
| authorized_by | FK users NULL | PIN apoteker (obat keras) / otorisasi diskon |

### `sale_item_batches` — batch yang terpakai (FEFO)
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| sale_item_id | FK | |
| batch_id | FK | |
| qty_base | INTEGER | |
| unit_cost_x100 | INTEGER | snapshot HPP → dasar laporan laba |

Satu `sale_items` bisa punya beberapa baris di sini. Setiap baris menghasilkan satu
`stock_movements` bertipe `SALE`. Void membuat movement `SALE_VOID` yang membalik
baris-baris ini, sehingga stok kembali ke batch asal.

### `sale_payments`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| sale_id | FK | |
| method | TEXT | `CASH` / `QRIS` / `DEBIT` |
| amount | INTEGER | bagian tagihan yang dibayar metode ini |
| tendered | INTEGER NULL | uang diterima (tunai) |
| change_amount | INTEGER NULL | kembalian |
| reference | TEXT NULL | kode approval EDC / QRIS |

**Pembayaran split** (misal sebagian tunai, sisanya QRIS): satu nota bisa punya beberapa baris.
Aturan:
- `SUM(amount) = sales.grand_total`.
- Maksimal satu baris `CASH`, dan hanya baris `CASH` yang boleh punya `tendered`/`change_amount`
  (kembalian hanya dari tunai).
- Non-tunai tidak boleh melebihi sisa tagihan.
- Uang tunai yang masuk ke laci = `amount` baris `CASH` (bukan `tendered`), itu yang dihitung
  ke `shifts.expected_cash`.

### Aturan tambahan (migration `014_sales.sql`)
- `sales.client_ref` (UNIQUE bila terisi): kunci idempotensi per checkout dari layar kasir.
- Satu resep hanya punya satu nota `COMPLETED` (index unik parsial `sales(prescription_id)`).
- `sales` tidak bisa dihapus; satu-satunya perubahan yang diizinkan `COMPLETED` → `VOID` tanpa
  mengubah angka. `sale_items`, `sale_item_batches`, `sale_payments` tidak bisa diubah/dihapus.
- Nota hanya bisa masuk ke shift `OPEN` (trigger); shift `CLOSED` final dan tidak bisa dihapus.
- Trigger `prescriptions_closed` diganti: resep `PAID` boleh kembali ke `SCREENED` (saat notanya void).

---

## 6. Resep

### `prescriptions`
| Kolom | Tipe | Keterangan |
|---|---|---|
| id | INTEGER PK | |
| prescription_number | TEXT | nomor resep dari dokter |
| prescription_date | TEXT | |
| doctor_id | FK | |
| customer_id | FK NULL | |
| patient_name, patient_age, patient_address | TEXT | snapshot (wajib untuk laporan narkotika) |
| number | TEXT UNIQUE | nomor internal `RSP-2609-0001` (`doc_sequences`), tidak bisa diubah |
| status | TEXT | `DRAFT` / `SCREENED` / `PAID` / `CANCELLED` |
| screened_by, screened_at, screening_note | FK users, TEXT, TEXT | skrining apoteker |
| created_by | FK users | |
| cancelled_at, cancelled_by, cancel_reason | | batal wajib beralasan |
| note | TEXT NULL | |

Trigger: `DELETE` ditolak; resep `PAID`/`CANCELLED` tidak bisa di-`UPDATE`. Mengubah resep
`SCREENED` mengembalikannya ke `DRAFT` (validasi dihapus).

### `prescription_items` — isi resep sebelum dibayar
Bentuk sama dengan `sale_items`: `item_kind` `PRODUCT` / `COMPOUND` / `SERVICE`, `parent_item_id`
(komponen racikan → baris `COMPOUND`), `product_id`, `product_unit_id`, `description` (snapshot nama),
`compound_form` (`POWDER`/`CAPSULE`/`OINTMENT`/`LIQUID`/`OTHER`, hanya racikan), `qty`, `conversion`,
`qty_base`, `unit_price` (sudah harga tier), `price_tier_id`, `line_total`, `usage_instruction`.
Nilai baris `COMPOUND` = jumlah `line_total` komponennya. Item tidak pernah di-`UPDATE`: saat resep
disimpan, item lama dihapus (komponen dulu) lalu ditulis ulang; ditolak bila resep sudah `PAID`/`CANCELLED`.
Stok belum berkurang di tahap ini.

Saat dibayar, item disalin ke `sale_items` (lewat `sales.prescription_id`) dan stok berkurang FEFO.
Laporan narkotika/psikotropika = `sale_items` join `products.drug_class`, ditambah
penerimaan dari `purchase_items`, dan stok awal/akhir dari kartu stok.

---

## 7. Retur (Tahap 2)

| Tabel | Kolom utama |
|---|---|
| `sale_returns` | number, sale_id, shift_id, returned_at, reason, refund_total, created_by, authorized_by |
| `sale_return_items` | sale_return_id, sale_item_batch_id, qty_base, refund_amount → movement `SALE_RETURN` ke batch asal |
| `supplier_returns` | number, supplier_id, purchase_id NULL, return_date, settlement (`DEBT_CUT` / `REPLACEMENT` / `REFUND`), total, created_by |
| `supplier_return_items` | supplier_return_id, batch_id, qty_base, amount → movement `SUPPLIER_RETURN` |

Validasi: jumlah retur per `sale_item_batch_id` tidak boleh melebihi jumlah terjual dikurangi
retur sebelumnya.

---

## 8. Gambaran relasi inti

```
products ─┬─ product_units ── product_barcodes
          ├─ batches ──┬── stock_movements (kartu stok)
          │            ├── sale_item_batches ── sale_items ── sales ─┬─ sale_payments
          │            │                                             ├─ shifts
          │            │                                             └─ prescriptions ── doctors
          │            └── purchase_items ── purchases ── suppliers
          └─ categories
```

## 9. Index penting

- `batches(product_id, expiry_date)` → FEFO, dashboard ED
- `stock_movements(product_id, id)` dan `(ref_type, ref_id)` → kartu stok, pelacakan dokumen
- `sales(sold_at)`, `sales(shift_id)` → laporan harian dan per shift
- `sale_items(sale_id)`, `sale_item_batches(sale_item_id)`, `sale_item_batches(batch_id)`
- `products(name)` + tabel FTS5 untuk pencarian nama/generik instan di kasir
- `purchases(due_date) WHERE payment_type = 'CREDIT'` → hutang jatuh tempo

## 10. Alur transaksi penjualan di database (satu `BEGIN IMMEDIATE ... COMMIT`)

1. Pastikan ada shift `OPEN`.
2. Ambil nomor dari `doc_sequences`.
3. Insert `sales`, lalu `sale_items`.
4. Per item: jalankan query FEFO, pecah `qty_base` ke beberapa batch, insert `sale_item_batches`
   dan `stock_movements` (trigger mengurangi stok; bila minus → rollback).
5. Insert `sale_payments`, lalu `audit_logs` bila ada otorisasi.
6. Commit, lalu cetak struk. Kegagalan cetak **tidak** membatalkan transaksi (ada cetak ulang).

Implementasi: `db::write_tx` (BEGIN IMMEDIATE → commit, rollback otomatis bila gagal) dipanggil
`sales::service::create_sale`. Sebelum commit, ringkasan stok setiap batch yang tersentuh dicocokkan
dengan `SUM` kartu stoknya. Test konkurensi (`src-tauri/src/sales/tests.rs`) menjalankan 8 checkout
bersamaan atas stok yang hanya cukup untuk 5 (lewat mutex bersama maupun koneksi terpisah ke file
yang sama): tepat 5 berhasil, sisanya ditolak, stok tidak pernah minus.

## 11. Keputusan

| Pertanyaan | Keputusan |
|---|---|
| Bahasa nama tabel/kolom | Inggris |
| Definisi batch | Lot per penerimaan (bukan unik per nomor batch) |
| Penjualan pecahan satuan terkecil | Tidak ada, qty selalu integer |
| Harga grosir/bertingkat | Ya, per jumlah per satuan jual (`price_tiers`) |
| Pembayaran split | Ya (`sale_payments` banyak baris per nota) |
| Peran | 4 peran (`OWNER`, `PHARMACIST`, `TECHNICIAN`, `CASHIER`), satu user bisa multi-peran (`user_roles`) |
| HPP & laba | Disembunyikan dari TTK dan kasir; untuk apoteker diatur pemilik |
