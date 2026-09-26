use serde::Serialize;
use ts_rs::TS;

/// Isi dashboard untuk user yang sedang login. Panel `null` = user tidak berhak melihatnya.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Dashboard {
    /// Tanggal hari ini `YYYY-MM-DD` (jam komputer kasir).
    pub today: String,
    /// Shift kasir & penjualan sendiri. Hak `SHIFT_MANAGE`.
    pub shift: Option<ShiftPanel>,
    /// Omzet seluruh apotek. Hak `REPORT_SALES`; laba hanya dengan `VIEW_COST`.
    pub sales: Option<SalesPanel>,
    /// Peringatan stok menipis & ED. Hak `STOCK_COUNT_INPUT`; nilai persediaan dengan `VIEW_COST`.
    pub stock: Option<StockPanel>,
    /// Antrian resep. Hak `PRESCRIPTION_INPUT`, `PRESCRIPTION_VALIDATE`, atau `SALE_CREATE`.
    pub prescriptions: Option<RxPanel>,
    /// Stok opname yang belum selesai. Hak `STOCK_COUNT_INPUT`.
    pub opname: Option<OpnamePanel>,
    /// Hutang supplier. Hak `SUPPLIER_DEBT_MANAGE`.
    pub debts: Option<DebtPanel>,
}

/// Jumlah nota dan nilainya (rupiah).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tally {
    pub count: i64,
    pub amount: i64,
}

// ─── Shift ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShiftPanel {
    /// Shift yang sedang terbuka (hanya satu per aplikasi), bila ada.
    pub open: Option<OpenShift>,
    /// Penjualan user ini hari ini (semua shift).
    pub my_sales_today: Tally,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenShift {
    pub id: i64,
    pub opened_by: String,
    pub opened_at: String,
    /// Shift ini dibuka oleh user yang sedang login.
    pub is_mine: bool,
    /// Angka shift hanya untuk pemilik shift atau user dengan `REPORT_SALES`.
    pub opening_cash: Option<i64>,
    pub sales: Option<Tally>,
    /// Uang tunai yang seharusnya ada di laci: modal + tunai masuk.
    pub expected_cash: Option<i64>,
}

// ─── Penjualan ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SalesPanel {
    pub today: Tally,
    pub yesterday: Tally,
    /// Bulan berjalan sampai hari ini.
    pub month: Tally,
    /// Nota yang dibatalkan hari ini.
    pub void_today: Tally,
    /// Penjualan resep hari ini (sudah termasuk di `today`).
    pub prescription_today: Tally,
    pub by_method_today: Vec<MethodTotal>,
    /// 7 hari terakhir termasuk hari ini, urut tanggal; hari tanpa penjualan bernilai 0.
    pub daily: Vec<DailyTotal>,
    /// Obat terlaris bulan ini menurut nilai penjualan.
    pub top_products: Vec<TopProduct>,
    /// Laba kotor (penjualan tanpa PPN − HPP batch). Hanya untuk `VIEW_COST`.
    pub gross_profit_today: Option<i64>,
    pub gross_profit_month: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MethodTotal {
    /// `CASH` / `QRIS` / `DEBIT`.
    pub method: String,
    pub amount: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DailyTotal {
    pub date: String,
    pub count: i64,
    pub amount: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TopProduct {
    pub product_id: i64,
    pub name: String,
    pub qty_base: i64,
    pub base_unit_name: String,
    pub amount: i64,
}

// ─── Stok ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockPanel {
    /// Obat aktif dengan stok di bawah stok minimal (termasuk yang kosong).
    pub low_count: i64,
    /// Obat aktif dengan stok nol.
    pub empty_count: i64,
    /// Batch ber-stok dengan ED ≤ 3 bulan dan belum lewat.
    pub near_expiry_count: i64,
    /// Batch ber-stok yang sudah lewat ED (terkunci otomatis, tidak bisa dijual).
    pub expired_count: i64,
    /// Batch sudah ED dulu, lalu yang hampir ED; urut ED terdekat.
    pub expiring: Vec<ExpiryRow>,
    /// Obat paling kritis (stok / minimal terkecil).
    pub low_stock: Vec<LowStockRow>,
    /// Nilai persediaan (Σ qty × HPP, rupiah). Hanya untuk `VIEW_COST`.
    pub inventory_value: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExpiryRow {
    pub batch_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub batch_number: String,
    pub expiry_date: String,
    pub qty_base: i64,
    pub base_unit_name: String,
    /// Sisa hari sampai ED; ≤ 0 berarti sudah ED.
    pub days_left: i64,
    pub is_locked: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LowStockRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub stock_base: i64,
    pub min_stock_base: i64,
    pub base_unit_name: String,
}

// ─── Resep ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RxPanel {
    /// Resep DRAFT, menunggu skrining apoteker.
    pub awaiting_screening: i64,
    /// Resep sudah divalidasi, menunggu dibayar di kasir.
    pub ready_to_pay: i64,
    /// Antrian yang relevan untuk user: `DRAFT` untuk yang input/validasi resep,
    /// `SCREENED` untuk kasir.
    pub queue_status: String,
    pub queue: Vec<RxQueueRow>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RxQueueRow {
    pub id: i64,
    pub number: String,
    pub prescription_date: String,
    pub patient_name: String,
    pub doctor_name: String,
    /// Jumlah item (obat, racikan, jasa) tanpa komponen racikan.
    pub item_count: i64,
    pub created_at: String,
}

// ─── Opname ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnamePanel {
    pub drafts: i64,
    pub awaiting_approval: i64,
    /// Stok awal sudah dikunci.
    pub opening_locked: bool,
    /// User boleh menyetujui opname (`STOCK_COUNT_APPROVE`).
    pub can_approve: bool,
    /// Opname DRAFT & SUBMITTED, yang menunggu persetujuan lebih dulu.
    pub pending: Vec<OpnamePendingRow>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnamePendingRow {
    pub id: i64,
    pub number: String,
    /// `OPENING` / `PERIODIC`.
    pub opname_type: String,
    /// `DRAFT` / `SUBMITTED`.
    pub status: String,
    pub scope_note: Option<String>,
    pub created_by: Option<String>,
    pub item_count: i64,
    pub created_at: String,
}

// ─── Hutang ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebtPanel {
    /// Seluruh faktur kredit yang belum lunas.
    pub outstanding: Tally,
    /// Sudah lewat jatuh tempo.
    pub overdue: Tally,
    /// Jatuh tempo hari ini sampai 7 hari ke depan.
    pub due_soon: Tally,
    /// Faktur belum lunas, jatuh tempo terdekat dulu.
    pub upcoming: Vec<DebtRow>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebtRow {
    pub purchase_id: i64,
    pub number: String,
    pub supplier_name: String,
    pub invoice_number: String,
    pub due_date: String,
    pub outstanding: i64,
    /// Sisa hari sampai jatuh tempo; negatif = terlambat.
    pub days_left: i64,
}
