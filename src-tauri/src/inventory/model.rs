use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

// ─── Stok per obat & batch ───────────────────────────────────────────────────

/// Filter daftar stok.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum StockFilter {
    /// Semua obat aktif.
    All,
    /// Stok di bawah stok minimal.
    Low,
    /// Stok nol.
    Empty,
    /// Punya batch ber-stok dengan ED ≤ 3 bulan (belum lewat).
    NearExpiry,
    /// Punya batch ber-stok yang sudah lewat ED.
    Expired,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockListQuery {
    /// Cari nama, generik, kode, atau barcode persis.
    pub q: Option<String>,
    pub category_id: Option<i64>,
    pub rack_id: Option<i64>,
    pub filter: StockFilter,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub base_unit_name: String,
    pub rack_name: Option<String>,
    pub min_stock_base: i64,
    /// Total stok semua batch (satuan terkecil).
    pub stock_base: i64,
    /// Stok yang bisa dijual: batch tidak terkunci dan belum lewat ED.
    pub sellable_base: i64,
    /// Stok di batch yang sudah lewat ED.
    pub expired_base: i64,
    /// ED terdekat di antara batch yang masih punya stok.
    pub nearest_expiry: Option<String>,
    pub batch_count: i64,
    /// Nilai persediaan (Σ qty × HPP, rupiah). Hanya untuk user dengan VIEW_COST.
    pub stock_value: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockPage {
    pub rows: Vec<StockRow>,
    pub total: i64,
    /// Tanggal hari ini menurut aplikasi (`YYYY-MM-DD`), dasar status ED di tampilan.
    pub today: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum BatchSource {
    Opening,
    Purchase,
    Adjustment,
}

text_enum!(BatchSource {
    Opening => "OPENING",
    Purchase => "PURCHASE",
    Adjustment => "ADJUSTMENT",
});

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchRow {
    pub id: i64,
    pub batch_number: String,
    pub expiry_date: String,
    pub qty_on_hand_base: i64,
    /// HPP per satuan terkecil (× 100). Hanya untuk user dengan VIEW_COST.
    pub unit_cost_x100: Option<i64>,
    pub is_locked: bool,
    pub lock_reason: Option<String>,
    pub source_type: BatchSource,
    pub created_at: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductBatches {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub base_unit_name: String,
    /// Termasuk batch yang stoknya sudah habis bila diminta.
    pub batches: Vec<BatchRow>,
}

// ─── Kartu stok ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum MovementType {
    Opening,
    Purchase,
    PurchaseVoid,
    Sale,
    SaleVoid,
    SaleReturn,
    SupplierReturn,
    Adjustment,
    Destruction,
}

text_enum!(MovementType {
    Opening => "OPENING",
    Purchase => "PURCHASE",
    PurchaseVoid => "PURCHASE_VOID",
    Sale => "SALE",
    SaleVoid => "SALE_VOID",
    SaleReturn => "SALE_RETURN",
    SupplierReturn => "SUPPLIER_RETURN",
    Adjustment => "ADJUSTMENT",
    Destruction => "DESTRUCTION",
});

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockCardQuery {
    pub product_id: i64,
    /// Hanya satu batch; saldo dihitung per batch.
    pub batch_id: Option<i64>,
    /// `YYYY-MM-DD`, inklusif.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockCardRow {
    pub id: i64,
    pub created_at: String,
    pub movement_type: MovementType,
    pub batch_id: i64,
    pub batch_number: String,
    pub expiry_date: String,
    /// Bertanda: + masuk, − keluar (satuan terkecil).
    pub qty_change_base: i64,
    /// Saldo setelah kejadian ini.
    pub balance_base: i64,
    pub ref_type: String,
    pub ref_id: i64,
    /// Nomor dokumen asal (misal nomor opname), bila dikenali.
    pub ref_number: Option<String>,
    pub username: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StockCardPage {
    pub rows: Vec<StockCardRow>,
    pub total: i64,
    /// Saldo sebelum `date_from` (0 bila tanpa tanggal awal).
    pub opening_balance: i64,
    /// Jumlah masuk / keluar dalam rentang tanggal (semua halaman).
    pub total_in: i64,
    pub total_out: i64,
    /// Saldo di akhir rentang tanggal.
    pub closing_balance: i64,
}

// ─── Stok opname ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum OpnameType {
    /// Stok awal (pengganti migrasi data): semua baris adalah batch baru.
    Opening,
    /// Opname berkala: hitung ulang batch yang ada, selisih menjadi penyesuaian.
    Periodic,
}

text_enum!(OpnameType {
    Opening => "OPENING",
    Periodic => "PERIODIC",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum OpnameStatus {
    Draft,
    Submitted,
    Approved,
    Cancelled,
}

text_enum!(OpnameStatus {
    Draft => "DRAFT",
    Submitted => "SUBMITTED",
    Approved => "APPROVED",
    Cancelled => "CANCELLED",
});

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnamePageQuery {
    /// Cari nomor atau keterangan.
    pub q: Option<String>,
    pub status: Option<OpnameStatus>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameRow {
    pub id: i64,
    pub number: String,
    pub opname_type: OpnameType,
    pub scope_note: Option<String>,
    pub status: OpnameStatus,
    pub item_count: i64,
    /// Baris yang sudah diisi hasil hitung fisik.
    pub counted_count: i64,
    pub created_at: String,
    pub created_by: Option<String>,
    pub approved_at: Option<String>,
    pub approved_by: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnamePage {
    pub rows: Vec<OpnameRow>,
    pub total: i64,
    /// Stok awal sudah dikunci: opname stok awal baru tidak bisa dibuat lagi.
    pub opening_locked: bool,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameCreateInput {
    pub opname_type: OpnameType,
    /// Cakupan, misal "Rak Obat Bebas A".
    pub scope_note: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameItem {
    pub id: i64,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub base_unit_name: String,
    /// `None` = batch baru (stok awal atau batch yang ditemukan saat opname).
    pub batch_id: Option<i64>,
    pub batch_number: String,
    pub expiry_date: String,
    /// HPP per satuan terkecil (× 100). Hanya untuk user dengan VIEW_COST.
    pub unit_cost_x100: Option<i64>,
    /// HPP sudah diisi (untuk user tanpa VIEW_COST yang tidak melihat nilainya).
    pub has_cost: bool,
    /// Snapshot stok sistem saat baris ditambahkan.
    pub system_qty_base: i64,
    pub physical_qty_base: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameDetail {
    pub header: OpnameRow,
    pub items: Vec<OpnameItem>,
    /// Nilai selisih (Σ selisih × HPP, rupiah). Hanya untuk user dengan VIEW_COST.
    pub difference_value: Option<i64>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameItemInput {
    pub opname_id: i64,
    /// `None` = baris baru.
    pub id: Option<i64>,
    pub product_id: i64,
    /// Batch yang dihitung ulang; `None` = batch baru.
    pub batch_id: Option<i64>,
    /// Wajib untuk batch baru.
    pub batch_number: Option<String>,
    /// `YYYY-MM-DD`, wajib untuk batch baru.
    pub expiry_date: Option<String>,
    /// HPP per satuan terkecil (× 100), wajib untuk batch baru. `None` saat mengubah baris =
    /// pertahankan HPP lama (user tanpa VIEW_COST tidak melihat nilainya).
    pub unit_cost_x100: Option<i64>,
    pub physical_qty_base: Option<i64>,
    pub note: Option<String>,
}

/// Isi opname berkala dengan semua batch ber-stok dalam cakupan (semua bila kosong).
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameFillInput {
    pub opname_id: i64,
    pub product_id: Option<i64>,
    pub rack_id: Option<i64>,
    pub category_id: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpnameSaveResult {
    pub opname: OpnameDetail,
    /// Peringatan yang tidak menggagalkan simpan, misal ED sudah lewat.
    pub warnings: Vec<String>,
}
