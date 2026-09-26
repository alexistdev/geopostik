use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::master::DrugClass;

/// Enum yang disimpan sebagai TEXT di SQLite dan dikirim sebagai string ke frontend.
macro_rules! text_enum {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                match value.as_str()? {
                    $($text => Ok($name::$variant),)+
                    _ => Err(FromSqlError::InvalidType),
                }
            }
        }
    };
}

// ─── Supplier ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Supplier {
    pub id: i64,
    /// Dibuat otomatis oleh sistem (SUP0001), tidak bisa diubah.
    pub code: String,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub npwp: Option<String>,
    /// Tempo pembayaran default (hari) untuk faktur kredit.
    pub payment_term_days: i64,
    pub is_active: bool,
    pub created_at: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupplierInput {
    pub id: Option<i64>,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub npwp: Option<String>,
    pub payment_term_days: i64,
    pub is_active: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupplierPage {
    pub rows: Vec<Supplier>,
    pub total: i64,
}

// ─── Faktur pembelian ────────────────────────────────────────────────────────

/// DRAFT (boleh setengah jadi) → POSTED (stok bertambah) → VOID (batal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum PurchaseStatus {
    Draft,
    Posted,
    Void,
}

text_enum!(PurchaseStatus {
    Draft => "DRAFT",
    Posted => "POSTED",
    Void => "VOID",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum PurchasePaymentType {
    Cash,
    /// Masuk daftar hutang supplier sampai lunas.
    Credit,
}

text_enum!(PurchasePaymentType {
    Cash => "CASH",
    Credit => "CREDIT",
});

/// Harga di faktur termasuk PPN, belum termasuk PPN, atau tanpa PPN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum TaxMode {
    Included,
    Excluded,
    None,
}

text_enum!(TaxMode {
    Included => "INCLUDED",
    Excluded => "EXCLUDED",
    None => "NONE",
});

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseQuery {
    /// Cari nomor internal, nomor faktur, atau nama supplier.
    pub q: Option<String>,
    pub status: Option<PurchaseStatus>,
    pub supplier_id: Option<i64>,
    /// Tanggal terima `YYYY-MM-DD`, inklusif.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseRow {
    pub id: i64,
    /// Nomor internal `PB-2609-0001`.
    pub number: String,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub received_date: String,
    pub due_date: Option<String>,
    pub payment_type: PurchasePaymentType,
    pub status: PurchaseStatus,
    pub item_count: i64,
    pub grand_total: i64,
    pub created_at: String,
    pub created_by: Option<String>,
    pub posted_at: Option<String>,
    /// Sisa hutang faktur kredit yang sudah diposting. Hanya untuk `SUPPLIER_DEBT_MANAGE`.
    pub outstanding: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchasePage {
    pub rows: Vec<PurchaseRow>,
    pub total: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseItemInput {
    /// Satuan beli (box/strip/...); obat ikut dari satuan ini.
    pub product_unit_id: i64,
    /// Jumlah dalam satuan beli.
    pub qty: i64,
    /// Bonus dalam satuan beli (tanpa harga).
    pub bonus_qty: i64,
    /// Harga beli per satuan beli sesuai faktur.
    pub unit_price: i64,
    pub discount1_bp: i64,
    pub discount2_bp: i64,
    pub batch_number: String,
    /// `YYYY-MM-DD`.
    pub expiry_date: String,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseInput {
    /// `None` = faktur baru.
    pub id: Option<i64>,
    pub supplier_id: i64,
    pub invoice_number: String,
    pub invoice_date: String,
    pub received_date: String,
    /// Wajib untuk kredit.
    pub due_date: Option<String>,
    pub payment_type: PurchasePaymentType,
    pub tax_mode: TaxMode,
    pub tax_rate_bp: i64,
    /// Diskon faktur tingkat header (rupiah).
    pub extra_discount: i64,
    pub note: Option<String>,
    pub items: Vec<PurchaseItemInput>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseItemDetail {
    pub id: i64,
    pub line_no: i64,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub drug_class: DrugClass,
    pub product_unit_id: i64,
    pub unit_name: String,
    pub base_unit_name: String,
    /// Isi satuan beli (snapshot).
    pub conversion: i64,
    pub qty: i64,
    pub bonus_qty: i64,
    pub unit_price: i64,
    pub discount1_bp: i64,
    pub discount2_bp: i64,
    /// Setelah diskon baris, sebelum diskon faktur & PPN.
    pub line_total: i64,
    pub batch_number: String,
    pub expiry_date: String,
    /// Stok yang masuk (qty + bonus) dalam satuan terkecil.
    pub qty_base: i64,
    /// HPP per satuan terkecil (× 100). Draft: perkiraan dengan pengaturan pajak saat ini.
    /// Hanya untuk `VIEW_COST`.
    pub unit_cost_x100: Option<i64>,
    /// HPP acuan obat sebelum faktur ini (pembanding harga naik). Hanya untuk `VIEW_COST`.
    pub last_cost_x100: Option<i64>,
    /// Batch yang dibuat saat posting.
    pub batch_id: Option<i64>,
    /// Satuan beli yang bisa dipilih (hanya untuk draft, agar baris bisa diubah).
    pub units: Vec<PurchaseUnit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum SupplierPaymentMethod {
    Cash,
    Transfer,
    Giro,
}

text_enum!(SupplierPaymentMethod {
    Cash => "CASH",
    Transfer => "TRANSFER",
    Giro => "GIRO",
});

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupplierPaymentRow {
    pub id: i64,
    /// Nomor bukti `BH-2609-0001`.
    pub number: String,
    pub purchase_id: i64,
    pub payment_date: String,
    pub amount: i64,
    pub method: SupplierPaymentMethod,
    /// No. transfer / giro.
    pub reference: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
    pub voided_at: Option<String>,
    pub voided_by: Option<String>,
    pub void_reason: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseDetail {
    pub id: i64,
    pub number: String,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub received_date: String,
    pub due_date: Option<String>,
    pub payment_type: PurchasePaymentType,
    pub tax_mode: TaxMode,
    pub tax_rate_bp: i64,
    pub status: PurchaseStatus,
    pub note: Option<String>,
    /// Σ qty × harga (sebelum diskon).
    pub subtotal: i64,
    pub extra_discount: i64,
    /// Diskon baris + diskon faktur.
    pub discount_total: i64,
    pub tax_total: i64,
    pub grand_total: i64,
    /// PPN masuk HPP (non-PKP). Draft: pengaturan saat ini; setelah posting: snapshot.
    pub tax_in_cost: bool,
    pub items: Vec<PurchaseItemDetail>,
    pub created_at: String,
    pub created_by: Option<String>,
    pub posted_at: Option<String>,
    pub posted_by: Option<String>,
    pub voided_at: Option<String>,
    pub voided_by: Option<String>,
    pub void_reason: Option<String>,
    /// Sisa hutang (kredit & diposting). Hanya untuk `SUPPLIER_DEBT_MANAGE`.
    pub outstanding: Option<i64>,
    /// Riwayat pembayaran (termasuk yang dibatalkan). Hanya untuk `SUPPLIER_DEBT_MANAGE`.
    pub payments: Option<Vec<SupplierPaymentRow>>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseSaveResult {
    pub purchase: PurchaseDetail,
    /// Peringatan yang tidak menggagalkan simpan, misal ED dekat atau harga beli naik.
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchasePostInput {
    pub id: i64,
    /// Perbarui HPP acuan obat dan hitung ulang harga jual `AUTO` dari margin.
    pub update_prices: bool,
}

/// Nilai awal form faktur.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseDefaults {
    /// Tarif PPN default (basis point).
    pub tax_rate_bp: i64,
    /// Apotek PKP: PPN tidak masuk HPP.
    pub is_pkp: bool,
    /// Batas "ED dekat" untuk peringatan (hari).
    pub near_expiry_days: i64,
    pub today: String,
}

/// Obat untuk baris faktur: satuan beli yang bisa dipilih.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseProduct {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub drug_class: DrugClass,
    pub base_unit_name: String,
    /// Urut isi terbesar dulu (box, strip, tablet).
    pub units: Vec<PurchaseUnit>,
    /// Satuan yang cocok dengan barcode yang di-scan.
    pub matched_unit_id: Option<i64>,
    /// HPP acuan per satuan terkecil (× 100). Hanya untuk `VIEW_COST`.
    pub last_cost_x100: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurchaseUnit {
    pub product_unit_id: i64,
    pub unit_name: String,
    pub conversion: i64,
}

// ─── Hutang supplier ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum DebtFilter {
    /// Belum lunas.
    Open,
    /// Belum lunas dan lewat jatuh tempo.
    Overdue,
    /// Belum lunas, jatuh tempo ≤ 7 hari lagi.
    DueSoon,
    Paid,
    All,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebtQuery {
    /// Cari nomor internal, nomor faktur, atau nama supplier.
    pub q: Option<String>,
    pub supplier_id: Option<i64>,
    pub filter: DebtFilter,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupplierDebtRow {
    pub purchase_id: i64,
    pub number: String,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: String,
    pub grand_total: i64,
    /// Pembayaran yang tidak dibatalkan.
    pub paid: i64,
    /// Retur ke supplier yang memotong hutang.
    pub returned: i64,
    pub outstanding: i64,
    /// Negatif = lewat jatuh tempo.
    pub days_left: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebtSummary {
    pub open_count: i64,
    pub open_amount: i64,
    pub overdue_count: i64,
    pub overdue_amount: i64,
    pub due_soon_count: i64,
    pub due_soon_amount: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebtPage {
    pub rows: Vec<SupplierDebtRow>,
    pub total: i64,
    /// Ringkasan semua hutang belum lunas (tanpa filter).
    pub summary: DebtSummary,
    pub today: String,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupplierPaymentInput {
    pub purchase_id: i64,
    /// `YYYY-MM-DD`.
    pub payment_date: String,
    pub amount: i64,
    pub method: SupplierPaymentMethod,
    pub reference: Option<String>,
    pub note: Option<String>,
}
