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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum DrugClass {
    Free,
    LimitedFree,
    Hard,
    Psychotropic,
    Narcotic,
}

text_enum!(DrugClass {
    Free => "FREE",
    LimitedFree => "LIMITED_FREE",
    Hard => "HARD",
    Psychotropic => "PSYCHOTROPIC",
    Narcotic => "NARCOTIC",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum PriceMode {
    Auto,
    Manual,
}

text_enum!(PriceMode {
    Auto => "AUTO",
    Manual => "MANUAL",
});

// ─── Kategori & satuan ───────────────────────────────────────────────────────

/// Jenis data di menu Master Data, untuk aksi bersama (aktif/nonaktif, hapus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum MasterKind {
    Category,
    Rack,
    Manufacturer,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Category {
    pub id: i64,
    /// Dibuat otomatis oleh sistem (KTG0001), tidak bisa diubah.
    pub code: String,
    pub name: String,
    /// `None` bila user tidak berhak melihat margin, atau kategori tidak punya margin sendiri.
    pub margin_bp: Option<i64>,
    pub is_active: bool,
    pub created_at: String,
    /// Username pembuat; `None` untuk data lama sebelum pembuat dicatat.
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CategoryInput {
    pub id: Option<i64>,
    pub name: String,
    pub margin_bp: Option<i64>,
    pub is_active: bool,
}

/// Master sederhana yang hanya berisi nama: rak dan pabrik.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NamedItem {
    pub id: i64,
    /// Dibuat otomatis oleh sistem (RAK0001 / PBR0001), tidak bisa diubah.
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    /// Username pembuat; `None` untuk data lama sebelum pembuat dicatat.
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NamedItemInput {
    pub id: Option<i64>,
    pub name: String,
    pub is_active: bool,
}

/// Query daftar Master Data per halaman.
#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MasterPageQuery {
    /// Cari nama atau kode (termasuk hasil scan label barcode).
    pub q: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CategoryPage {
    pub rows: Vec<Category>,
    pub total: i64,
    /// Margin global yang dipakai kategori tanpa margin sendiri. `None` bila user tidak berhak melihat margin.
    pub default_margin_bp: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NamedItemPage {
    pub rows: Vec<NamedItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Unit {
    pub id: i64,
    pub name: String,
}

// ─── Daftar obat ─────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductListQuery {
    /// Cari nama, nama generik, kode, atau barcode persis.
    pub q: Option<String>,
    pub category_id: Option<i64>,
    pub drug_class: Option<DrugClass>,
    pub include_inactive: bool,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductListRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub category_name: Option<String>,
    pub drug_class: DrugClass,
    pub is_owa: bool,
    pub base_unit_name: String,
    pub sale_unit_name: Option<String>,
    pub sale_price: Option<i64>,
    /// Total stok semua batch, dalam satuan terkecil.
    pub stock_base: i64,
    pub min_stock_base: i64,
    pub rack_name: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductListResult {
    pub rows: Vec<ProductListRow>,
    pub total: i64,
}

// ─── Detail obat ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductDetail {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub manufacturer_id: Option<i64>,
    pub category_id: Option<i64>,
    pub drug_class: DrugClass,
    pub is_owa: bool,
    pub base_unit_id: i64,
    pub min_stock_base: i64,
    pub rack_id: Option<i64>,
    pub is_active: bool,
    /// Sudah punya batch → satuan dasar & konversi satuan lama tidak bisa diubah.
    pub has_stock: bool,
    /// Margin milik obat ini (bukan warisan). Hanya untuk user yang boleh melihat/mengatur harga.
    pub margin_bp: Option<i64>,
    /// Margin yang berlaku (obat → kategori → default). Hanya untuk user yang boleh melihat/mengatur harga.
    pub effective_margin_bp: Option<i64>,
    /// HPP acuan per satuan terkecil (× 100). Hanya untuk user dengan VIEW_COST.
    pub last_cost_x100: Option<i64>,
    pub units: Vec<ProductUnitDetail>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductUnitDetail {
    pub id: i64,
    pub unit_id: i64,
    pub unit_name: String,
    pub conversion: i64,
    pub sell_price: i64,
    pub price_mode: PriceMode,
    pub is_default_sale: bool,
    pub is_active: bool,
    pub barcodes: Vec<String>,
    pub tiers: Vec<PriceTierDetail>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PriceTierDetail {
    pub id: i64,
    pub min_qty: i64,
    pub price_mode: PriceMode,
    /// Hanya untuk user yang boleh melihat/mengatur harga.
    pub margin_bp: Option<i64>,
    pub price: i64,
}

// ─── Simpan data obat (PRODUCT_MANAGE) ───────────────────────────────────────

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductInput {
    pub id: Option<i64>,
    /// Kosong → dibuat otomatis (OBT00001).
    pub code: Option<String>,
    pub name: String,
    pub generic_name: Option<String>,
    pub manufacturer_id: Option<i64>,
    pub category_id: Option<i64>,
    pub drug_class: DrugClass,
    pub is_owa: bool,
    pub base_unit_id: i64,
    pub min_stock_base: i64,
    pub rack_id: Option<i64>,
    /// Semua satuan aktif. Satuan lama yang tidak dikirim akan dinonaktifkan.
    pub units: Vec<ProductUnitInput>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductUnitInput {
    pub id: Option<i64>,
    pub unit_id: i64,
    pub conversion: i64,
    pub is_default_sale: bool,
    pub barcodes: Vec<String>,
}

// ─── Simpan harga (PRICE_MANAGE) ─────────────────────────────────────────────

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductPricesInput {
    pub product_id: i64,
    /// Margin milik obat; `None` = ikut kategori/default.
    pub margin_bp: Option<i64>,
    /// HPP acuan per satuan terkecil (× 100). Hanya diterapkan untuk user dengan VIEW_COST;
    /// nantinya diperbarui otomatis oleh penerimaan barang.
    pub last_cost_x100: Option<i64>,
    pub units: Vec<UnitPriceInput>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UnitPriceInput {
    pub product_unit_id: i64,
    pub price_mode: PriceMode,
    /// Dipakai bila `MANUAL`; diabaikan bila `AUTO`.
    pub sell_price: i64,
    /// Semua tier aktif satuan ini. Tier lama yang tidak dikirim akan dinonaktifkan.
    pub tiers: Vec<PriceTierInput>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PriceTierInput {
    /// Tier dikenali lewat jumlah minimal per satuan.
    pub min_qty: i64,
    pub price_mode: PriceMode,
    /// Wajib bila `AUTO`.
    pub margin_bp: Option<i64>,
    /// Dipakai bila `MANUAL`.
    pub price: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProductSaveResult {
    pub product: ProductDetail,
    /// Peringatan yang tidak menggagalkan simpan, misal harga di bawah HPP.
    pub warnings: Vec<String>,
}
