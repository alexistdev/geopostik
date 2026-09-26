use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Rentang tanggal laporan, keduanya `YYYY-MM-DD` dan inklusif.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportRange {
    pub from: String,
    pub to: String,
}

// ─── Penjualan ───────────────────────────────────────────────────────────────

/// Laporan penjualan per periode. Tanpa `REPORT_SALES` hanya berisi nota user sendiri.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SalesReport {
    pub from: String,
    pub to: String,
    /// `true` = hanya penjualan user yang sedang login (hak `REPORT_SALES_OWN_SHIFT`).
    pub own_only: bool,
    pub summary: SalesReportSummary,
    /// Setiap tanggal dalam rentang, termasuk yang tanpa penjualan.
    pub daily: Vec<ReportDay>,
    pub by_method: Vec<ReportMethod>,
    pub by_cashier: Vec<ReportCashier>,
    pub shifts: Vec<ReportShift>,
}

#[derive(Debug, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SalesReportSummary {
    /// Nota selesai (tidak batal).
    pub count: i64,
    /// Σ subtotal sebelum diskon nota.
    pub subtotal: i64,
    pub discount: i64,
    pub tax: i64,
    pub rounding: i64,
    /// Omzet = Σ grand total.
    pub net: i64,
    pub prescription_count: i64,
    pub prescription_amount: i64,
    /// Nota batal, menurut tanggal pembatalan.
    pub void_count: i64,
    pub void_amount: i64,
    /// HPP batch terpakai & laba kotor (penjualan tanpa PPN − HPP). Hanya untuk `VIEW_COST`.
    pub cost: Option<i64>,
    pub gross_profit: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportDay {
    pub date: String,
    pub count: i64,
    pub amount: i64,
    /// Hanya untuk `VIEW_COST`.
    pub gross_profit: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportMethod {
    /// `CASH` / `QRIS` / `DEBIT`.
    pub method: String,
    pub count: i64,
    pub amount: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportCashier {
    pub user_id: i64,
    pub name: String,
    pub count: i64,
    pub amount: i64,
}

/// Shift yang dibuka dalam rentang. Penjualan = nota yang terlihat oleh user (lihat `own_only`).
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportShift {
    pub id: i64,
    pub opened_by: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    /// `OPEN` / `CLOSED`.
    pub status: String,
    pub count: i64,
    pub amount: i64,
    /// Angka kas hanya untuk pemilik shift atau `REPORT_SALES`.
    pub opening_cash: Option<i64>,
    pub expected_cash: Option<i64>,
    pub counted_cash: Option<i64>,
    pub difference: Option<i64>,
}

// ─── Obat terlaris & slow moving ─────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductReport {
    pub from: String,
    pub to: String,
    /// Obat terjual, nilai terbesar dulu.
    pub top: Vec<ReportProductRow>,
    /// Obat aktif yang masih ada stok tetapi tidak terjual dalam rentang.
    pub slow: Vec<ReportSlowRow>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportProductRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub qty_base: i64,
    pub base_unit_name: String,
    /// Jumlah nota yang memuat obat ini.
    pub sale_count: i64,
    pub amount: i64,
    /// HPP & laba kotor baris (tanpa PPN). Hanya untuk `VIEW_COST`.
    pub cost: Option<i64>,
    pub gross_profit: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportSlowRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub stock_base: i64,
    pub base_unit_name: String,
    /// Tanggal terakhir terjual (kapan pun), `null` bila belum pernah.
    pub last_sold_at: Option<String>,
    /// Nilai stok (Σ qty × HPP). Hanya untuk `VIEW_COST`.
    pub value: Option<i64>,
}

// ─── Nilai persediaan ────────────────────────────────────────────────────────

/// Hak `VIEW_COST`.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InventoryReport {
    pub total_value: i64,
    pub product_count: i64,
    pub batch_count: i64,
    /// Nilai batch yang sudah lewat ED (sudah termasuk di `total_value`).
    pub expired_value: i64,
    pub by_category: Vec<ReportCategoryValue>,
    /// Obat dengan stok, nilai terbesar dulu.
    pub rows: Vec<ReportInventoryRow>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportCategoryValue {
    /// `null` = tanpa kategori.
    pub category: Option<String>,
    pub product_count: i64,
    pub value: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportInventoryRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub category: Option<String>,
    pub stock_base: i64,
    pub base_unit_name: String,
    pub batch_count: i64,
    pub value: i64,
}

// ─── ED ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExpiryReportQuery {
    /// Batch dengan ED sampai sekian hari ke depan ikut ditampilkan (1–730).
    pub days: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExpiryReport {
    pub today: String,
    pub expired_count: i64,
    pub near_count: i64,
    /// Nilai HPP batch sudah ED / hampir ED. Hanya untuk `VIEW_COST`.
    pub expired_value: Option<i64>,
    pub near_value: Option<i64>,
    /// Sudah ED dulu, lalu ED terdekat.
    pub rows: Vec<ReportExpiryRow>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportExpiryRow {
    pub batch_id: i64,
    pub product_id: i64,
    pub code: String,
    pub product_name: String,
    pub batch_number: String,
    pub expiry_date: String,
    pub qty_base: i64,
    pub base_unit_name: String,
    /// ≤ 0 berarti sudah ED.
    pub days_left: i64,
    pub is_locked: bool,
    pub value: Option<i64>,
}

// ─── Pembelian ───────────────────────────────────────────────────────────────

/// Faktur `POSTED` menurut tanggal terima. Hak `PURCHASE_RECEIVE`.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseReport {
    pub from: String,
    pub to: String,
    pub count: i64,
    pub subtotal: i64,
    pub discount: i64,
    pub tax: i64,
    pub total: i64,
    pub cash_total: i64,
    pub credit_total: i64,
    pub by_supplier: Vec<ReportSupplierRow>,
    pub invoices: Vec<ReportInvoiceRow>,
    /// Sisa hutang faktur ini hanya dikirim untuk `SUPPLIER_DEBT_MANAGE`.
    pub shows_debt: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportSupplierRow {
    pub supplier_id: i64,
    pub name: String,
    pub count: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportInvoiceRow {
    pub purchase_id: i64,
    pub number: String,
    pub supplier_name: String,
    pub invoice_number: String,
    pub received_date: String,
    /// `CASH` / `CREDIT`.
    pub payment_type: String,
    pub due_date: Option<String>,
    pub total: i64,
    /// Sisa hutang (faktur kredit). Hanya untuk `SUPPLIER_DEBT_MANAGE`.
    pub outstanding: Option<i64>,
}

// ─── SIPNAP ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SipnapQuery {
    pub year: i32,
    /// 1–12.
    pub month: u32,
}

/// Mutasi narkotika & psikotropika sebulan dari kartu stok. Hak `REPORT_SIPNAP`.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SipnapReport {
    pub year: i32,
    pub month: u32,
    pub rows: Vec<SipnapRow>,
}

/// Semua jumlah dalam satuan dasar. `closing = opening + received − sold − destroyed + adjusted`.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SipnapRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    /// `NARCOTIC` / `PSYCHOTROPIC`.
    pub drug_class: String,
    pub base_unit_name: String,
    pub opening: i64,
    /// Stok awal, pembelian (dikurangi batal beli).
    pub received: i64,
    /// Penjualan bersih (dikurangi batal jual dan retur penjualan).
    pub sold: i64,
    /// Pemusnahan dan retur ke supplier.
    pub destroyed: i64,
    /// Penyesuaian opname (±).
    pub adjusted: i64,
    pub closing: i64,
}
