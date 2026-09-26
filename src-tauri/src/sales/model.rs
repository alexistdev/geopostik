use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::master::DrugClass;

/// Metode bayar. Non-tunai dicatat manual (tanpa integrasi bank).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum PaymentMethod {
    Cash,
    Qris,
    Debit,
}

impl PaymentMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "CASH",
            PaymentMethod::Qris => "QRIS",
            PaymentMethod::Debit => "DEBIT",
        }
    }
}

impl ToSql for PaymentMethod {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for PaymentMethod {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "CASH" => Ok(PaymentMethod::Cash),
            "QRIS" => Ok(PaymentMethod::Qris),
            "DEBIT" => Ok(PaymentMethod::Debit),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

// ─── Status kasir & shift ────────────────────────────────────────────────────

/// Keadaan layar kasir: shift terbuka dan aturan yang dipakai saat menghitung total.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PosState {
    pub shift: Option<ShiftSummary>,
    /// Diskon per baris maksimal (basis point dari harga baris) tanpa otorisasi PIN.
    pub max_discount_bp: i64,
    /// Total nota dibulatkan ke bawah ke kelipatan ini (rupiah); 0 = tanpa pembulatan.
    pub total_rounding: i64,
    /// Resep yang sudah divalidasi apoteker dan menunggu dibayar.
    pub prescriptions_ready: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShiftSummary {
    pub id: i64,
    pub opened_by_id: i64,
    pub opened_by: String,
    pub opened_at: String,
    pub is_mine: bool,
    /// `OPEN` / `CLOSED`.
    pub status: String,
    pub closed_at: Option<String>,
    /// Angka shift hanya untuk pemilik shift atau user dengan `REPORT_SALES`.
    pub figures: Option<ShiftFigures>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShiftFigures {
    pub opening_cash: i64,
    pub sale_count: i64,
    pub sales_total: i64,
    pub void_count: i64,
    /// Bagian tunai dari nota yang tidak batal.
    pub cash_in: i64,
    /// Modal + tunai masuk = uang yang seharusnya ada di laci.
    pub expected_cash: i64,
    pub by_method: Vec<MethodAmount>,
    /// Terisi setelah shift ditutup.
    pub counted_cash: Option<i64>,
    pub difference: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MethodAmount {
    pub method: PaymentMethod,
    pub amount: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShiftOpenInput {
    pub opening_cash: i64,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShiftCloseInput {
    /// Shift yang ditampilkan di layar; ditolak bila ternyata sudah berganti.
    pub shift_id: i64,
    pub counted_cash: i64,
    pub note: Option<String>,
}

// ─── Pencarian obat ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PosProduct {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub drug_class: DrugClass,
    pub is_owa: bool,
    pub base_unit_name: String,
    /// Stok yang bisa dijual (belum ED, tidak terkunci), satuan terkecil.
    pub sellable_base: i64,
    pub units: Vec<PosUnit>,
    /// Satuan yang cocok dengan barcode yang di-scan.
    pub matched_unit_id: Option<i64>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PosUnit {
    pub product_unit_id: i64,
    pub unit_name: String,
    pub conversion: i64,
    pub sell_price: i64,
    pub is_default: bool,
    /// Harga grosir, `minQty` naik.
    pub tiers: Vec<PosTier>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PosTier {
    pub min_qty: i64,
    pub price: i64,
}

// ─── Penjualan ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleInput {
    /// Kunci idempotensi dari layar kasir (UUID), tetap sama saat kirim ulang checkout yang sama.
    pub client_ref: String,
    /// Diisi untuk membayar resep berstatus `SCREENED`; `items` diabaikan (diambil dari resep).
    pub prescription_id: Option<i64>,
    pub items: Vec<SaleItemInput>,
    pub payments: Vec<PaymentInput>,
    /// Total yang dilihat kasir. Ditolak bila berbeda dengan hitungan sistem (misal harga baru diubah).
    pub expected_total: i64,
    /// PIN apoteker untuk obat keras, bila kasir sendiri tidak berhak.
    pub hard_drug_pin: Option<String>,
    /// PIN untuk diskon di atas batas, bila kasir sendiri tidak berhak.
    pub discount_pin: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleItemInput {
    pub product_unit_id: i64,
    pub qty: i64,
    /// Diskon rupiah untuk seluruh baris.
    pub discount_amount: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaymentInput {
    pub method: PaymentMethod,
    /// Bagian tagihan yang dibayar dengan metode ini.
    pub amount: i64,
    /// Uang diterima (tunai saja); kembalian = tendered − amount.
    pub tendered: Option<i64>,
    /// Kode approval EDC / QRIS.
    pub reference: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleDetail {
    pub id: i64,
    pub number: String,
    pub shift_id: i64,
    pub cashier_name: String,
    pub sold_at: String,
    /// `OTC` / `PRESCRIPTION`.
    pub sale_type: String,
    pub prescription_id: Option<i64>,
    pub prescription_number: Option<String>,
    pub patient_name: Option<String>,
    pub subtotal: i64,
    pub discount_total: i64,
    pub rounding: i64,
    pub grand_total: i64,
    /// `COMPLETED` / `VOID`.
    pub status: String,
    pub voided_at: Option<String>,
    pub voided_by: Option<String>,
    pub void_reason: Option<String>,
    pub items: Vec<SaleItemDetail>,
    pub payments: Vec<PaymentDetail>,
    pub change_amount: i64,
    /// Nota ini sudah tersimpan sebelumnya dengan `clientRef` yang sama (kiriman ulang).
    pub replayed: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleItemDetail {
    pub id: i64,
    pub line_no: i64,
    /// `PRODUCT` / `COMPOUND` / `SERVICE`.
    pub kind: String,
    /// Komponen racikan menunjuk ke baris `COMPOUND`.
    pub parent_item_id: Option<i64>,
    pub description: String,
    pub unit_name: Option<String>,
    pub qty: i64,
    pub unit_price: i64,
    pub tier_min_qty: Option<i64>,
    pub discount_amount: i64,
    pub line_total: i64,
    pub usage_instruction: Option<String>,
    /// Batch yang terpakai (FEFO), untuk penelusuran.
    pub batches: Vec<SaleBatch>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleBatch {
    pub batch_number: String,
    pub expiry_date: String,
    pub qty_base: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaymentDetail {
    pub method: PaymentMethod,
    pub amount: i64,
    pub tendered: Option<i64>,
    pub change_amount: Option<i64>,
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleQuery {
    /// Kosong = shift yang sedang terbuka.
    pub shift_id: Option<i64>,
    /// Cari nomor nota.
    pub q: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleRow {
    pub id: i64,
    pub number: String,
    pub sold_at: String,
    pub cashier_name: String,
    pub sale_type: String,
    pub item_count: i64,
    pub grand_total: i64,
    pub status: String,
    /// Metode bayar, misal "CASH+QRIS".
    pub methods: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SalePage {
    pub rows: Vec<SaleRow>,
    pub total: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaleVoidInput {
    pub sale_id: i64,
    pub reason: String,
    /// PIN Pemilik/Apoteker, bila user sendiri tidak berhak membatalkan.
    pub pin: Option<String>,
}

// ─── Resep di kasir ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PosPrescription {
    pub id: i64,
    pub number: String,
    pub prescription_number: String,
    pub prescription_date: String,
    pub patient_name: String,
    pub doctor_name: String,
    pub screened_at: Option<String>,
    pub item_count: i64,
}

/// Hitungan harga final resep saat akan dibayar (harga terbaru, sudah harga tier).
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionQuote {
    pub id: i64,
    pub number: String,
    pub prescription_number: String,
    pub patient_name: String,
    pub doctor_name: String,
    pub lines: Vec<QuoteLine>,
    pub subtotal: i64,
    pub rounding: i64,
    pub grand_total: i64,
    /// Stok kurang, obat nonaktif, dll. Resep dengan peringatan tidak bisa dibayar.
    pub problems: Vec<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuoteLine {
    pub kind: String,
    pub description: String,
    pub unit_name: Option<String>,
    pub qty: i64,
    pub unit_price: i64,
    pub line_total: i64,
    pub components: Vec<QuoteLine>,
}
