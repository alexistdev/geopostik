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

// ─── Dokter ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Doctor {
    pub id: i64,
    /// Dibuat otomatis oleh sistem (DOK0001), tidak bisa diubah.
    pub code: String,
    pub name: String,
    /// Nomor Surat Izin Praktik.
    pub sip_number: Option<String>,
    pub specialty: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DoctorInput {
    pub id: Option<i64>,
    pub name: String,
    pub sip_number: Option<String>,
    pub specialty: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DoctorPage {
    pub rows: Vec<Doctor>,
    pub total: i64,
}

// ─── Pasien (tabel `customers`) ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Gender {
    #[serde(rename = "M")]
    Male,
    #[serde(rename = "F")]
    Female,
}

text_enum!(Gender {
    Male => "M",
    Female => "F",
});

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Patient {
    pub id: i64,
    /// Dibuat otomatis oleh sistem (PSN0001), tidak bisa diubah.
    pub code: String,
    pub name: String,
    pub gender: Option<Gender>,
    /// `YYYY-MM-DD`.
    pub birth_date: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PatientInput {
    pub id: Option<i64>,
    pub name: String,
    pub gender: Option<Gender>,
    pub birth_date: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PatientPage {
    pub rows: Vec<Patient>,
    pub total: i64,
}

// ─── Resep ───────────────────────────────────────────────────────────────────

/// DRAFT (menunggu skrining) → SCREENED (siap dibayar) → PAID (dibayar di kasir).
/// Resep yang diubah setelah divalidasi kembali ke DRAFT. Batal = CANCELLED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum PrescriptionStatus {
    Draft,
    Screened,
    Paid,
    Cancelled,
}

text_enum!(PrescriptionStatus {
    Draft => "DRAFT",
    Screened => "SCREENED",
    Paid => "PAID",
    Cancelled => "CANCELLED",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ItemKind {
    /// Obat biasa, atau komponen racikan.
    Product,
    /// Racikan (puyer, kapsul, salep, ...) berisi beberapa komponen obat.
    Compound,
    /// Jasa racik, embalase, dll.
    Service,
}

text_enum!(ItemKind {
    Product => "PRODUCT",
    Compound => "COMPOUND",
    Service => "SERVICE",
});

/// Bentuk sediaan racikan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum CompoundForm {
    Powder,
    Capsule,
    Ointment,
    Liquid,
    Other,
}

text_enum!(CompoundForm {
    Powder => "POWDER",
    Capsule => "CAPSULE",
    Ointment => "OINTMENT",
    Liquid => "LIQUID",
    Other => "OTHER",
});

#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionQuery {
    /// Cari nomor internal, nomor resep dokter, nama pasien, atau nama dokter.
    pub q: Option<String>,
    pub status: Option<PrescriptionStatus>,
    /// Tanggal resep `YYYY-MM-DD`, inklusif.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionRow {
    pub id: i64,
    pub number: String,
    pub prescription_number: String,
    pub prescription_date: String,
    pub doctor_name: String,
    pub patient_name: String,
    pub patient_age: Option<String>,
    /// Jumlah baris utama (obat, racikan, jasa).
    pub item_count: i64,
    pub total: i64,
    /// Berisi narkotika / psikotropika.
    pub has_controlled: bool,
    pub status: PrescriptionStatus,
    pub created_at: String,
    pub created_by: Option<String>,
    pub screened_by: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionPage {
    pub rows: Vec<PrescriptionRow>,
    pub total: i64,
    /// Jumlah resep menunggu skrining (semua tanggal), untuk penanda di tab.
    pub draft_count: i64,
    /// Jumlah resep siap dibayar (semua tanggal).
    pub screened_count: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionDetail {
    pub id: i64,
    pub number: String,
    pub prescription_number: String,
    pub prescription_date: String,
    pub doctor_id: i64,
    pub doctor_name: String,
    pub doctor_sip_number: Option<String>,
    pub customer_id: Option<i64>,
    pub patient_name: String,
    pub patient_age: Option<String>,
    pub patient_address: Option<String>,
    pub note: Option<String>,
    pub status: PrescriptionStatus,
    pub screened_by: Option<String>,
    pub screened_at: Option<String>,
    pub screening_note: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
    pub cancelled_at: Option<String>,
    pub cancelled_by: Option<String>,
    pub cancel_reason: Option<String>,
    pub items: Vec<PrescriptionItemDetail>,
    /// Jumlah semua baris utama (perkiraan; harga final dihitung saat dibayar).
    pub total: i64,
    /// Peringatan yang tidak menghalangi, misal stok kurang.
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionItemDetail {
    pub id: i64,
    pub line_no: i64,
    pub kind: ItemKind,
    pub product_id: Option<i64>,
    pub product_unit_id: Option<i64>,
    pub product_code: Option<String>,
    pub unit_name: Option<String>,
    pub drug_class: Option<DrugClass>,
    pub description: String,
    pub compound_form: Option<CompoundForm>,
    pub qty: i64,
    pub conversion: i64,
    pub qty_base: i64,
    pub unit_price: i64,
    /// Tier harga grosir yang dipakai (jumlah minimalnya); `None` = harga eceran.
    pub tier_min_qty: Option<i64>,
    pub line_total: i64,
    pub usage_instruction: Option<String>,
    /// Stok yang bisa dijual (belum ED, tidak terkunci), dalam satuan terkecil. Hanya untuk obat.
    pub stock_base: Option<i64>,
    /// Komponen racikan (hanya untuk `COMPOUND`).
    pub components: Vec<PrescriptionItemDetail>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionInput {
    pub id: Option<i64>,
    /// Nomor resep yang ditulis dokter.
    pub prescription_number: String,
    /// `YYYY-MM-DD`.
    pub prescription_date: String,
    pub doctor_id: i64,
    /// Pasien terdaftar (opsional). Nama/umur/alamat tetap disimpan sebagai snapshot.
    pub customer_id: Option<i64>,
    pub patient_name: String,
    pub patient_age: Option<String>,
    pub patient_address: Option<String>,
    pub note: Option<String>,
    pub items: Vec<PrescriptionItemInput>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrescriptionItemInput {
    pub kind: ItemKind,
    /// Wajib untuk `PRODUCT`.
    pub product_unit_id: Option<i64>,
    /// Jumlah dalam satuan jual (obat), jumlah bungkus/kapsul/pot (racikan), atau 1 (jasa).
    pub qty: i64,
    /// Wajib untuk racikan dan jasa; untuk obat diisi otomatis nama obat.
    pub description: Option<String>,
    /// Wajib untuk racikan.
    pub compound_form: Option<CompoundForm>,
    /// Tarif per jumlah, hanya untuk jasa. Harga obat selalu dihitung sistem.
    pub unit_price: Option<i64>,
    /// Aturan pakai (signa) untuk etiket, misal "3 x sehari 1 tablet sesudah makan".
    pub usage_instruction: Option<String>,
    /// Komponen racikan: jumlah total untuk seluruh racikan.
    pub components: Vec<CompoundComponentInput>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompoundComponentInput {
    pub product_unit_id: i64,
    pub qty: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScreeningInput {
    pub id: i64,
    pub note: Option<String>,
}
