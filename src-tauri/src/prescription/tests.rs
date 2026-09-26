use rusqlite::Connection;

use super::model::*;
use super::pricing::price_for_qty;
use super::repo::PersonTable;
use super::service;
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions};
use crate::db;
use crate::error::AppError;
use crate::master::MasterPageQuery;

const PCT_TABLET: i64 = 1; // product_units.id
const PCT_STRIP: i64 = 2;
const CODEINE_TABLET: i64 = 3;
const CTM_TABLET: i64 = 4;

/// Paracetamol (bebas, tablet Rp500, strip isi 10 Rp4.500, strip ≥ 5 Rp4.000, stok 100 tablet),
/// Codein (narkotika), CTM (bebas, tanpa stok).
fn setup() -> Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'apt', 'Apt. Rina', 'x');
         INSERT INTO products (id, code, name, drug_class, base_unit_id) VALUES
             (1, 'OBT0001', 'Paracetamol 500 mg', 'FREE', 1),
             (2, 'OBT0002', 'Codein 10 mg', 'NARCOTIC', 1),
             (3, 'OBT0003', 'CTM 4 mg', 'FREE', 1);
         INSERT INTO product_units (id, product_id, unit_id, conversion, sell_price, is_default_sale) VALUES
             (1, 1, 1, 1, 500, 0), (2, 1, 4, 10, 4500, 1), (3, 2, 1, 1, 2000, 1), (4, 3, 1, 1, 300, 1);
         INSERT INTO price_tiers (product_unit_id, min_qty, price) VALUES (2, 5, 4000);
         INSERT INTO batches (id, product_id, batch_number, expiry_date, unit_cost_x100, source_type)
             VALUES (1, 1, 'B1', '2099-01-01', 30000, 'OPENING'), (2, 2, 'C1', '2099-01-01', 100000, 'OPENING');
         INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id)
             VALUES (1, 1, 'OPENING', 100, 'stock_opname', 1, 1), (2, 2, 'OPENING', 50, 'stock_opname', 1, 1);",
    )
    .unwrap();
    conn
}

fn user(roles: &[Role]) -> SessionUser {
    SessionUser {
        id: 1,
        username: "apt".into(),
        full_name: "Apt. Rina".into(),
        roles: roles.to_vec(),
        permissions: effective_permissions(roles, AccessSettings::default()),
    }
}

fn doctor(conn: &Connection, name: &str, sip: Option<&str>) -> Doctor {
    service::save_doctor(
        conn,
        &user(&[Role::Pharmacist]),
        &DoctorInput {
            id: None,
            name: name.into(),
            sip_number: sip.map(Into::into),
            specialty: None,
            address: None,
            phone: None,
            is_active: true,
        },
    )
    .unwrap()
}

fn product(unit: i64, qty: i64, usage: &str) -> PrescriptionItemInput {
    PrescriptionItemInput {
        kind: ItemKind::Product,
        product_unit_id: Some(unit),
        qty,
        description: None,
        compound_form: None,
        unit_price: None,
        usage_instruction: Some(usage.into()),
        components: vec![],
    }
}

fn compound(name: &str, qty: i64, components: &[(i64, i64)]) -> PrescriptionItemInput {
    PrescriptionItemInput {
        kind: ItemKind::Compound,
        product_unit_id: None,
        qty,
        description: Some(name.into()),
        compound_form: Some(CompoundForm::Powder),
        unit_price: None,
        usage_instruction: Some("3 x sehari 1 bungkus".into()),
        components: components
            .iter()
            .map(|&(product_unit_id, qty)| CompoundComponentInput { product_unit_id, qty })
            .collect(),
    }
}

fn service_line(name: &str, price: i64) -> PrescriptionItemInput {
    PrescriptionItemInput {
        kind: ItemKind::Service,
        product_unit_id: None,
        qty: 1,
        description: Some(name.into()),
        compound_form: None,
        unit_price: Some(price),
        usage_instruction: None,
        components: vec![],
    }
}

fn input(doctor_id: i64, items: Vec<PrescriptionItemInput>) -> PrescriptionInput {
    PrescriptionInput {
        id: None,
        prescription_number: "R/123".into(),
        prescription_date: "2026-01-15".into(),
        doctor_id,
        customer_id: None,
        patient_name: "Budi".into(),
        patient_age: Some("7 th".into()),
        patient_address: None,
        note: None,
        items,
    }
}

#[test]
fn tier_price_follows_quantity() {
    let conn = setup();
    assert_eq!(price_for_qty(&conn, PCT_STRIP, 4).unwrap().unit_price, 4500);
    let tier = price_for_qty(&conn, PCT_STRIP, 5).unwrap();
    assert_eq!((tier.unit_price, tier.tier_min_qty), (4000, Some(5)));
    assert!(price_for_qty(&conn, PCT_TABLET, 100).unwrap().tier_id.is_none());
}

#[test]
fn doctor_gets_code_and_unique_sip() {
    let conn = setup();
    let a = doctor(&conn, "dr. Andi", Some("SIP-1"));
    let b = doctor(&conn, "dr. Budi", None);
    assert_eq!((a.code.as_str(), b.code.as_str()), ("DOK0001", "DOK0002"));

    let dup = service::save_doctor(
        &conn,
        &user(&[Role::Technician]),
        &DoctorInput {
            id: None,
            name: "dr. Citra".into(),
            sip_number: Some("sip-1".into()),
            specialty: None,
            address: None,
            phone: None,
            is_active: true,
        },
    );
    assert!(matches!(dup, Err(AppError::Conflict(_))));

    let page = service::page_doctors(&conn, &MasterPageQuery { q: Some("sip-1".into()), offset: 0, limit: 25 }).unwrap();
    assert_eq!(page.total, 1);
}

#[test]
fn saves_products_compound_and_services() {
    let mut conn = setup();
    let d = doctor(&conn, "dr. Andi", None);
    let saved = service::save_prescription(
        &mut conn,
        &user(&[Role::Technician]),
        &input(
            d.id,
            vec![
                product(PCT_STRIP, 5, "3 x sehari 1 tablet"),
                compound("Puyer batuk", 10, &[(PCT_TABLET, 5), (CTM_TABLET, 5)]),
                service_line("Jasa racik", 5000),
            ],
        ),
    )
    .unwrap();

    assert!(saved.number.starts_with("RSP-"), "{}", saved.number);
    assert_eq!(saved.status, PrescriptionStatus::Draft);
    assert_eq!(saved.items.len(), 3);

    // 5 strip kena tier Rp4.000.
    let strip = &saved.items[0];
    assert_eq!((strip.unit_price, strip.line_total, strip.qty_base), (4000, 20_000, 50));
    assert_eq!(strip.tier_min_qty, Some(5));

    // Racikan = jumlah komponen: 5 × 500 + 5 × 300.
    let puyer = &saved.items[1];
    assert_eq!(puyer.components.len(), 2);
    assert_eq!(puyer.line_total, 4_000);
    assert_eq!(saved.total, 20_000 + 4_000 + 5_000);

    // Stok Paracetamol 100 tablet cukup untuk 55; CTM tidak punya stok → peringatan.
    assert_eq!(saved.warnings.len(), 1);
    assert!(saved.warnings[0].contains("CTM"), "{:?}", saved.warnings);

    let page = service::page_prescriptions(&conn, &PrescriptionQuery { q: Some("budi".into()), limit: 25, ..Default::default() }).unwrap();
    assert_eq!((page.total, page.draft_count), (1, 1));
    assert_eq!((page.rows[0].item_count, page.rows[0].total), (3, 29_000));
}

#[test]
fn controlled_drug_requires_patient_address() {
    let mut conn = setup();
    let d = doctor(&conn, "dr. Andi", None);
    let mut rx = input(d.id, vec![product(CODEINE_TABLET, 10, "2 x sehari")]);
    let err = service::save_prescription(&mut conn, &user(&[Role::Pharmacist]), &rx);
    assert!(matches!(err, Err(AppError::Validation(_))));

    rx.patient_address = Some("Jl. Mawar 1".into());
    let saved = service::save_prescription(&mut conn, &user(&[Role::Pharmacist]), &rx).unwrap();
    let page = service::page_prescriptions(&conn, &PrescriptionQuery { limit: 25, ..Default::default() }).unwrap();
    assert!(page.rows[0].has_controlled);
    assert_eq!(saved.items[0].drug_class, Some(crate::master::DrugClass::Narcotic));
}

#[test]
fn rejects_invalid_input() {
    let mut conn = setup();
    let d = doctor(&conn, "dr. Andi", None);
    let u = user(&[Role::Technician]);

    let empty = input(d.id, vec![]);
    assert!(service::save_prescription(&mut conn, &u, &empty).is_err());

    let mut future = input(d.id, vec![product(PCT_TABLET, 1, "")]);
    future.prescription_date = "2999-01-01".into();
    assert!(service::save_prescription(&mut conn, &u, &future).is_err());

    let no_components = input(d.id, vec![compound("Kosong", 10, &[])]);
    assert!(service::save_prescription(&mut conn, &u, &no_components).is_err());

    let zero = input(d.id, vec![product(PCT_TABLET, 0, "")]);
    assert!(service::save_prescription(&mut conn, &u, &zero).is_err());

    service::set_person_active(&conn, &u, PersonTable::Doctors, d.id, false).unwrap();
    let inactive_doctor = input(d.id, vec![product(PCT_TABLET, 1, "")]);
    assert!(service::save_prescription(&mut conn, &u, &inactive_doctor).is_err());
}

#[test]
fn screening_edit_and_cancel_flow() {
    let mut conn = setup();
    let d = doctor(&conn, "dr. Andi", None);
    let pharmacist = user(&[Role::Pharmacist]);
    let mut rx = input(d.id, vec![product(PCT_TABLET, 10, "3 x sehari")]);
    let saved = service::save_prescription(&mut conn, &pharmacist, &rx).unwrap();

    let screened = service::screen_prescription(
        &mut conn,
        &pharmacist,
        &ScreeningInput { id: saved.id, note: Some("Dosis sesuai".into()) },
    )
    .unwrap();
    assert_eq!(screened.status, PrescriptionStatus::Screened);
    assert_eq!(screened.screened_by.as_deref(), Some("Apt. Rina"));

    // Divalidasi dua kali ditolak.
    let again = service::screen_prescription(&mut conn, &pharmacist, &ScreeningInput { id: saved.id, note: None });
    assert!(matches!(again, Err(AppError::Conflict(_))));

    // Diubah setelah validasi → kembali menunggu skrining, nomor internal tetap.
    rx.id = Some(saved.id);
    rx.items = vec![product(PCT_TABLET, 20, "3 x sehari")];
    let edited = service::save_prescription(&mut conn, &pharmacist, &rx).unwrap();
    assert_eq!(edited.status, PrescriptionStatus::Draft);
    assert_eq!(edited.number, saved.number);
    assert!(edited.screened_by.is_none());
    assert_eq!(edited.items[0].qty, 20);

    // Batal wajib beralasan; resep batal tidak bisa diubah.
    assert!(service::cancel_prescription(&mut conn, &pharmacist, saved.id, " ").is_err());
    let cancelled = service::cancel_prescription(&mut conn, &pharmacist, saved.id, "Pasien tidak jadi menebus").unwrap();
    assert_eq!(cancelled.status, PrescriptionStatus::Cancelled);
    assert!(cancelled.warnings.is_empty());
    assert!(matches!(service::save_prescription(&mut conn, &pharmacist, &rx), Err(AppError::Conflict(_))));
    assert!(conn.execute("UPDATE prescriptions SET note = 'x'", []).is_err());
    assert!(conn.execute("DELETE FROM prescription_items", []).is_err());
    assert!(conn.execute("DELETE FROM prescriptions", []).is_err());

    // Semua langkah tercatat di log audit.
    let actions: Vec<String> = conn
        .prepare("SELECT action FROM audit_logs WHERE entity = 'prescriptions' ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(actions, ["CREATE", "SCREEN", "UPDATE", "CANCEL"]);
}

#[test]
fn used_doctor_and_patient_cannot_be_deleted() {
    let mut conn = setup();
    let u = user(&[Role::Owner]);
    let d = doctor(&conn, "dr. Andi", None);
    let p = service::save_patient(
        &conn,
        &u,
        &PatientInput {
            id: None,
            name: "Budi".into(),
            gender: Some(Gender::Male),
            birth_date: Some("2019-05-01".into()),
            address: None,
            phone: None,
            is_active: true,
        },
    )
    .unwrap();
    assert_eq!(p.code, "PSN0001");

    let mut rx = input(d.id, vec![product(PCT_TABLET, 1, "")]);
    rx.customer_id = Some(p.id);
    service::save_prescription(&mut conn, &u, &rx).unwrap();

    assert!(matches!(service::delete_person(&mut conn, &u, PersonTable::Doctors, d.id), Err(AppError::Conflict(_))));
    assert!(matches!(service::delete_person(&mut conn, &u, PersonTable::Patients, p.id), Err(AppError::Conflict(_))));

    let unused = doctor(&conn, "dr. Lain", None);
    service::delete_person(&mut conn, &u, PersonTable::Doctors, unused.id).unwrap();
    assert_eq!(service::list_doctors(&conn).unwrap().len(), 1);
    // Kode yang sudah dipakai tidak dipakai ulang.
    assert_eq!(doctor(&conn, "dr. Baru", None).code, "DOK0003");
    assert!(conn.execute("DELETE FROM doctors", []).is_err());
}
