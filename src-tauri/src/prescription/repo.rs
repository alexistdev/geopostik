use rusqlite::{Connection, OptionalExtension, named_params, params};

use super::model::{
    CompoundForm, Doctor, Gender, ItemKind, Patient, PrescriptionQuery, PrescriptionRow, PrescriptionStatus,
};
use crate::error::AppResult;
use crate::master::{DrugClass, MasterPageQuery, like_pattern};

fn page_bounds(limit: i64, offset: i64) -> (i64, i64) {
    (limit.clamp(1, 500), offset.max(0))
}

/// Tabel master resep yang punya kode otomatis. Nama tabel berasal dari konstanta, bukan input user.
#[derive(Debug, Clone, Copy)]
pub enum PersonTable {
    Doctors,
    Patients,
}

impl PersonTable {
    pub fn table(self) -> &'static str {
        match self {
            PersonTable::Doctors => "doctors",
            PersonTable::Patients => "customers",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            PersonTable::Doctors => "DOK",
            PersonTable::Patients => "PSN",
        }
    }

    /// Kolom di `prescriptions` yang menunjuk ke tabel ini.
    fn prescription_column(self) -> &'static str {
        match self {
            PersonTable::Doctors => "doctor_id",
            PersonTable::Patients => "customer_id",
        }
    }
}

/// Kode otomatis berikutnya (DOK0001 / PSN0001). Kode yang pernah dipakai, termasuk milik data
/// yang sudah dihapus, dilewati.
pub fn next_code(conn: &Connection, t: PersonTable) -> AppResult<String> {
    let mut n: i64 = conn.query_row(&format!("SELECT COALESCE(MAX(id), 0) + 1 FROM {}", t.table()), [], |r| r.get(0))?;
    loop {
        let code = format!("{}{n:04}", t.prefix());
        let used: bool = conn.query_row(
            &format!("SELECT EXISTS (SELECT 1 FROM {} WHERE code = ?1 COLLATE NOCASE)", t.table()),
            [&code],
            |r| r.get(0),
        )?;
        if !used {
            return Ok(code);
        }
        n += 1;
    }
}

pub fn set_active(conn: &Connection, t: PersonTable, id: i64, active: bool) -> AppResult<usize> {
    Ok(conn.execute(
        &format!(
            "UPDATE {} SET is_active = ?2, updated_at = datetime('now', 'localtime')
             WHERE id = ?1 AND deleted_at IS NULL",
            t.table()
        ),
        params![id, active],
    )?)
}

/// Jumlah resep (semua status) yang memakai dokter/pasien ini.
pub fn usage(conn: &Connection, t: PersonTable, id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        &format!("SELECT count(*) FROM prescriptions WHERE {} = ?1", t.prescription_column()),
        [id],
        |r| r.get(0),
    )?)
}

pub fn soft_delete(conn: &Connection, t: PersonTable, id: i64, user_id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        &format!(
            "UPDATE {} SET deleted_at = datetime('now', 'localtime'), deleted_by = ?2, is_active = 0,
                          updated_at = datetime('now', 'localtime')
             WHERE id = ?1 AND deleted_at IS NULL",
            t.table()
        ),
        params![id, user_id],
    )?)
}

// ─── Dokter ──────────────────────────────────────────────────────────────────

const DOCTOR_COLUMNS: &str = "id, code, name, sip_number, specialty, address, phone, is_active, created_at,
     (SELECT u.username FROM users u WHERE u.id = doctors.created_by)";

fn doctor_row(r: &rusqlite::Row) -> rusqlite::Result<Doctor> {
    Ok(Doctor {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        sip_number: r.get(3)?,
        specialty: r.get(4)?,
        address: r.get(5)?,
        phone: r.get(6)?,
        is_active: r.get(7)?,
        created_at: r.get(8)?,
        created_by: r.get(9)?,
    })
}

const DOCTOR_FILTER: &str = "deleted_at IS NULL
      AND (:like IS NULL OR name LIKE :like ESCAPE '\\' OR code LIKE :like ESCAPE '\\'
           OR sip_number LIKE :like ESCAPE '\\' OR specialty LIKE :like ESCAPE '\\')";

pub fn list_doctors(conn: &Connection) -> AppResult<Vec<Doctor>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {DOCTOR_COLUMNS} FROM doctors WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE, id"
    ))?;
    let rows = stmt.query_map([], doctor_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn page_doctors(conn: &Connection, q: &MasterPageQuery) -> AppResult<(Vec<Doctor>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let total = conn.query_row(
        &format!("SELECT count(*) FROM doctors WHERE {DOCTOR_FILTER}"),
        named_params! { ":like": like },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {DOCTOR_COLUMNS} FROM doctors WHERE {DOCTOR_FILTER}
         ORDER BY name COLLATE NOCASE, id LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":like": like, ":limit": limit, ":offset": offset }, doctor_row)?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn get_doctor(conn: &Connection, id: i64) -> AppResult<Option<Doctor>> {
    Ok(conn
        .query_row(
            &format!("SELECT {DOCTOR_COLUMNS} FROM doctors WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            doctor_row,
        )
        .optional()?)
}

pub struct DoctorFields<'a> {
    pub name: &'a str,
    pub sip_number: Option<&'a str>,
    pub specialty: Option<&'a str>,
    pub address: Option<&'a str>,
    pub phone: Option<&'a str>,
    pub is_active: bool,
}

pub fn insert_doctor(conn: &Connection, code: &str, f: &DoctorFields<'_>, created_by: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO doctors (code, name, sip_number, specialty, address, phone, is_active, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![code, f.name, f.sip_number, f.specialty, f.address, f.phone, f.is_active, created_by],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_doctor(conn: &Connection, id: i64, f: &DoctorFields<'_>) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE doctors SET name = ?2, sip_number = ?3, specialty = ?4, address = ?5, phone = ?6, is_active = ?7,
                            updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, f.name, f.sip_number, f.specialty, f.address, f.phone, f.is_active],
    )?)
}

/// Dokter lain (belum dihapus) dengan nomor SIP yang sama.
pub fn doctor_sip_taken(conn: &Connection, sip: &str, except_id: Option<i64>) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT name FROM doctors
             WHERE sip_number = ?1 COLLATE NOCASE AND id IS NOT ?2 AND deleted_at IS NULL",
            params![sip, except_id],
            |r| r.get(0),
        )
        .optional()?)
}

/// Dokter aktif yang belum dihapus, untuk dipilih di resep.
pub fn doctor_selectable(conn: &Connection, id: i64) -> AppResult<Option<bool>> {
    Ok(conn
        .query_row("SELECT is_active FROM doctors WHERE id = ?1 AND deleted_at IS NULL", [id], |r| r.get(0))
        .optional()?)
}

// ─── Pasien ──────────────────────────────────────────────────────────────────

const PATIENT_COLUMNS: &str = "id, code, name, gender, birth_date, address, phone, is_active, created_at,
     (SELECT u.username FROM users u WHERE u.id = customers.created_by)";

fn patient_row(r: &rusqlite::Row) -> rusqlite::Result<Patient> {
    Ok(Patient {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        gender: r.get::<_, Option<Gender>>(3)?,
        birth_date: r.get(4)?,
        address: r.get(5)?,
        phone: r.get(6)?,
        is_active: r.get(7)?,
        created_at: r.get(8)?,
        created_by: r.get(9)?,
    })
}

const PATIENT_FILTER: &str = "deleted_at IS NULL
      AND (:active_only = 0 OR is_active = 1)
      AND (:like IS NULL OR name LIKE :like ESCAPE '\\' OR code LIKE :like ESCAPE '\\'
           OR phone LIKE :like ESCAPE '\\')";

/// Pasien bisa sangat banyak, jadi selalu dipaginasi (juga untuk pencarian di form resep).
pub fn page_patients(conn: &Connection, q: &MasterPageQuery, active_only: bool) -> AppResult<(Vec<Patient>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let total = conn.query_row(
        &format!("SELECT count(*) FROM customers WHERE {PATIENT_FILTER}"),
        named_params! { ":like": like, ":active_only": active_only },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {PATIENT_COLUMNS} FROM customers WHERE {PATIENT_FILTER}
         ORDER BY name COLLATE NOCASE, id LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! { ":like": like, ":active_only": active_only, ":limit": limit, ":offset": offset },
            patient_row,
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn get_patient(conn: &Connection, id: i64) -> AppResult<Option<Patient>> {
    Ok(conn
        .query_row(
            &format!("SELECT {PATIENT_COLUMNS} FROM customers WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            patient_row,
        )
        .optional()?)
}

pub struct PatientFields<'a> {
    pub name: &'a str,
    pub gender: Option<Gender>,
    pub birth_date: Option<&'a str>,
    pub address: Option<&'a str>,
    pub phone: Option<&'a str>,
    pub is_active: bool,
}

pub fn insert_patient(conn: &Connection, code: &str, f: &PatientFields<'_>, created_by: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO customers (code, name, gender, birth_date, address, phone, is_active, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![code, f.name, f.gender, f.birth_date, f.address, f.phone, f.is_active, created_by],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_patient(conn: &Connection, id: i64, f: &PatientFields<'_>) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE customers SET name = ?2, gender = ?3, birth_date = ?4, address = ?5, phone = ?6, is_active = ?7,
                              updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, f.name, f.gender, f.birth_date, f.address, f.phone, f.is_active],
    )?)
}

pub fn patient_exists(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM customers WHERE id = ?1 AND deleted_at IS NULL)",
        [id],
        |r| r.get(0),
    )?)
}

// ─── Resep ───────────────────────────────────────────────────────────────────

const PRESCRIPTION_FILTER: &str = "
    FROM prescriptions p
    JOIN doctors d ON d.id = p.doctor_id
    WHERE (:status IS NULL OR p.status = :status)
      AND (:date_from IS NULL OR p.prescription_date >= :date_from)
      AND (:date_to IS NULL OR p.prescription_date <= :date_to)
      AND (:like IS NULL OR p.number LIKE :like ESCAPE '\\' OR p.prescription_number LIKE :like ESCAPE '\\'
           OR p.patient_name LIKE :like ESCAPE '\\' OR d.name LIKE :like ESCAPE '\\')";

pub fn page_prescriptions(conn: &Connection, q: &PrescriptionQuery) -> AppResult<(Vec<PrescriptionRow>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let date_from = q.date_from.as_deref().filter(|d| !d.is_empty());
    let date_to = q.date_to.as_deref().filter(|d| !d.is_empty());
    let total = conn.query_row(
        &format!("SELECT count(*) {PRESCRIPTION_FILTER}"),
        named_params! { ":status": q.status, ":date_from": date_from, ":date_to": date_to, ":like": like },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT p.id, p.number, p.prescription_number, p.prescription_date, d.name, p.patient_name, p.patient_age,
                (SELECT count(*) FROM prescription_items i WHERE i.prescription_id = p.id AND i.parent_item_id IS NULL),
                (SELECT COALESCE(SUM(i.line_total), 0) FROM prescription_items i
                 WHERE i.prescription_id = p.id AND i.parent_item_id IS NULL),
                EXISTS (SELECT 1 FROM prescription_items i JOIN products pr ON pr.id = i.product_id
                        WHERE i.prescription_id = p.id AND pr.drug_class IN ('PSYCHOTROPIC', 'NARCOTIC')),
                p.status, p.created_at,
                (SELECT u.full_name FROM users u WHERE u.id = p.created_by),
                (SELECT u.full_name FROM users u WHERE u.id = p.screened_by)
         {PRESCRIPTION_FILTER}
         ORDER BY p.id DESC LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! {
                ":status": q.status, ":date_from": date_from, ":date_to": date_to, ":like": like,
                ":limit": limit, ":offset": offset,
            },
            |r| {
                Ok(PrescriptionRow {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    prescription_number: r.get(2)?,
                    prescription_date: r.get(3)?,
                    doctor_name: r.get(4)?,
                    patient_name: r.get(5)?,
                    patient_age: r.get(6)?,
                    item_count: r.get(7)?,
                    total: r.get(8)?,
                    has_controlled: r.get(9)?,
                    status: r.get(10)?,
                    created_at: r.get(11)?,
                    created_by: r.get(12)?,
                    screened_by: r.get(13)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn count_by_status(conn: &Connection, status: PrescriptionStatus) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM prescriptions WHERE status = ?1", [status], |r| r.get(0))?)
}

pub struct HeaderRow {
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
}

pub fn get_header(conn: &Connection, id: i64) -> AppResult<Option<HeaderRow>> {
    Ok(conn
        .query_row(
            "SELECT p.id, p.number, p.prescription_number, p.prescription_date, p.doctor_id, d.name, d.sip_number,
                    p.customer_id, p.patient_name, p.patient_age, p.patient_address, p.note, p.status,
                    (SELECT u.full_name FROM users u WHERE u.id = p.screened_by), p.screened_at, p.screening_note,
                    p.created_at, (SELECT u.full_name FROM users u WHERE u.id = p.created_by),
                    p.cancelled_at, (SELECT u.full_name FROM users u WHERE u.id = p.cancelled_by), p.cancel_reason
             FROM prescriptions p JOIN doctors d ON d.id = p.doctor_id
             WHERE p.id = ?1",
            [id],
            |r| {
                Ok(HeaderRow {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    prescription_number: r.get(2)?,
                    prescription_date: r.get(3)?,
                    doctor_id: r.get(4)?,
                    doctor_name: r.get(5)?,
                    doctor_sip_number: r.get(6)?,
                    customer_id: r.get(7)?,
                    patient_name: r.get(8)?,
                    patient_age: r.get(9)?,
                    patient_address: r.get(10)?,
                    note: r.get(11)?,
                    status: r.get(12)?,
                    screened_by: r.get(13)?,
                    screened_at: r.get(14)?,
                    screening_note: r.get(15)?,
                    created_at: r.get(16)?,
                    created_by: r.get(17)?,
                    cancelled_at: r.get(18)?,
                    cancelled_by: r.get(19)?,
                    cancel_reason: r.get(20)?,
                })
            },
        )
        .optional()?)
}

pub fn status_of(conn: &Connection, id: i64) -> AppResult<Option<PrescriptionStatus>> {
    Ok(conn
        .query_row("SELECT status FROM prescriptions WHERE id = ?1", [id], |r| r.get(0))
        .optional()?)
}

pub struct HeaderFields<'a> {
    pub prescription_number: &'a str,
    pub prescription_date: &'a str,
    pub doctor_id: i64,
    pub customer_id: Option<i64>,
    pub patient_name: &'a str,
    pub patient_age: Option<&'a str>,
    pub patient_address: Option<&'a str>,
    pub note: Option<&'a str>,
}

pub fn insert_header(conn: &Connection, number: &str, f: &HeaderFields<'_>, created_by: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO prescriptions (number, prescription_number, prescription_date, doctor_id, customer_id,
                                    patient_name, patient_age, patient_address, note, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            number, f.prescription_number, f.prescription_date, f.doctor_id, f.customer_id,
            f.patient_name, f.patient_age, f.patient_address, f.note, created_by,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Ubah data resep. Resep yang sudah divalidasi kembali menunggu skrining.
pub fn update_header(conn: &Connection, id: i64, f: &HeaderFields<'_>) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE prescriptions SET prescription_number = ?2, prescription_date = ?3, doctor_id = ?4, customer_id = ?5,
                                  patient_name = ?6, patient_age = ?7, patient_address = ?8, note = ?9,
                                  status = 'DRAFT', screened_by = NULL, screened_at = NULL, screening_note = NULL,
                                  updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status IN ('DRAFT', 'SCREENED')",
        params![
            id, f.prescription_number, f.prescription_date, f.doctor_id, f.customer_id,
            f.patient_name, f.patient_age, f.patient_address, f.note,
        ],
    )?)
}

pub fn mark_screened(conn: &Connection, id: i64, user_id: i64, note: Option<&str>) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE prescriptions SET status = 'SCREENED', screened_by = ?2, screened_at = datetime('now', 'localtime'),
                                  screening_note = ?3, updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status = 'DRAFT'",
        params![id, user_id, note],
    )?)
}

pub fn mark_cancelled(conn: &Connection, id: i64, user_id: i64, reason: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE prescriptions SET status = 'CANCELLED', cancelled_by = ?2, cancelled_at = datetime('now', 'localtime'),
                                  cancel_reason = ?3, updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status IN ('DRAFT', 'SCREENED')",
        params![id, user_id, reason],
    )?)
}

/// Hapus semua item (komponen racikan lebih dulu karena menunjuk ke induknya).
pub fn delete_items(conn: &Connection, prescription_id: i64) -> AppResult<()> {
    conn.execute(
        "DELETE FROM prescription_items WHERE prescription_id = ?1 AND parent_item_id IS NOT NULL",
        [prescription_id],
    )?;
    conn.execute("DELETE FROM prescription_items WHERE prescription_id = ?1", [prescription_id])?;
    Ok(())
}

pub struct ItemFields<'a> {
    pub line_no: i64,
    pub kind: ItemKind,
    pub parent_item_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_unit_id: Option<i64>,
    pub description: &'a str,
    pub compound_form: Option<CompoundForm>,
    pub qty: i64,
    pub conversion: i64,
    pub unit_price: i64,
    pub price_tier_id: Option<i64>,
    pub line_total: i64,
    pub usage_instruction: Option<&'a str>,
}

pub fn insert_item(conn: &Connection, prescription_id: i64, f: &ItemFields<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO prescription_items (prescription_id, line_no, item_kind, parent_item_id, product_id,
                                         product_unit_id, description, compound_form, qty, conversion, qty_base,
                                         unit_price, price_tier_id, line_total, usage_instruction)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            prescription_id, f.line_no, f.kind, f.parent_item_id, f.product_id, f.product_unit_id, f.description,
            f.compound_form, f.qty, f.conversion, f.qty * f.conversion, f.unit_price, f.price_tier_id,
            f.line_total, f.usage_instruction,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct ItemRow {
    pub id: i64,
    pub line_no: i64,
    pub kind: ItemKind,
    pub parent_item_id: Option<i64>,
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
    pub tier_min_qty: Option<i64>,
    pub line_total: i64,
    pub usage_instruction: Option<String>,
}

pub fn items(conn: &Connection, prescription_id: i64) -> AppResult<Vec<ItemRow>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.line_no, i.item_kind, i.parent_item_id, i.product_id, i.product_unit_id, p.code, un.name,
                p.drug_class, i.description, i.compound_form, i.qty, i.conversion, i.qty_base, i.unit_price,
                t.min_qty, i.line_total, i.usage_instruction
         FROM prescription_items i
         LEFT JOIN products p ON p.id = i.product_id
         LEFT JOIN product_units pu ON pu.id = i.product_unit_id
         LEFT JOIN units un ON un.id = pu.unit_id
         LEFT JOIN price_tiers t ON t.id = i.price_tier_id
         WHERE i.prescription_id = ?1
         ORDER BY i.line_no, i.id",
    )?;
    let rows = stmt
        .query_map([prescription_id], |r| {
            Ok(ItemRow {
                id: r.get(0)?,
                line_no: r.get(1)?,
                kind: r.get(2)?,
                parent_item_id: r.get(3)?,
                product_id: r.get(4)?,
                product_unit_id: r.get(5)?,
                product_code: r.get(6)?,
                unit_name: r.get(7)?,
                drug_class: r.get(8)?,
                description: r.get(9)?,
                compound_form: r.get(10)?,
                qty: r.get(11)?,
                conversion: r.get(12)?,
                qty_base: r.get(13)?,
                unit_price: r.get(14)?,
                tier_min_qty: r.get(15)?,
                line_total: r.get(16)?,
                usage_instruction: r.get(17)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Data satuan jual yang dipilih di resep, untuk validasi dan snapshot.
pub struct SaleUnit {
    pub product_id: i64,
    pub product_name: String,
    pub unit_name: String,
    pub conversion: i64,
    pub drug_class: DrugClass,
    /// Obat & satuannya aktif dan belum dihapus.
    pub usable: bool,
}

pub fn sale_unit(conn: &Connection, product_unit_id: i64) -> AppResult<Option<SaleUnit>> {
    Ok(conn
        .query_row(
            "SELECT p.id, p.name, un.name, pu.conversion, p.drug_class,
                    p.is_active = 1 AND p.deleted_at IS NULL AND pu.is_active = 1
             FROM product_units pu
             JOIN products p ON p.id = pu.product_id
             JOIN units un ON un.id = pu.unit_id
             WHERE pu.id = ?1",
            [product_unit_id],
            |r| {
                Ok(SaleUnit {
                    product_id: r.get(0)?,
                    product_name: r.get(1)?,
                    unit_name: r.get(2)?,
                    conversion: r.get(3)?,
                    drug_class: r.get(4)?,
                    usable: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Stok yang bisa dijual: belum ED, tidak terkunci (sama dengan syarat FEFO), satuan terkecil.
pub fn sellable_stock(conn: &Connection, product_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(qty_on_hand_base), 0) FROM batches
         WHERE product_id = ?1 AND qty_on_hand_base > 0 AND is_locked = 0
           AND expiry_date > date('now', 'localtime')",
        [product_id],
        |r| r.get(0),
    )?)
}
