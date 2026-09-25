use rusqlite::Connection;

use super::model::*;
use super::repo::NamedTable;
use super::service::{self, PriceAccess};
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions};
use crate::db;
use crate::error::AppError;

const TABLET: i64 = 1; // urutan data awal di 001_init.sql
const STRIP: i64 = 4;
const BOX: i64 = 6;

fn setup() -> Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'u', 'U', 'x')",
        [],
    )
    .unwrap();
    conn
}

fn user(roles: &[Role]) -> SessionUser {
    SessionUser {
        id: 1,
        username: "u".into(),
        full_name: "U".into(),
        roles: roles.to_vec(),
        permissions: effective_permissions(roles, AccessSettings::default()),
    }
}

fn unit(unit_id: i64, conversion: i64, default: bool, barcodes: &[&str]) -> ProductUnitInput {
    ProductUnitInput {
        id: None,
        unit_id,
        conversion,
        is_default_sale: default,
        barcodes: barcodes.iter().map(|b| b.to_string()).collect(),
    }
}

fn paracetamol() -> ProductInput {
    ProductInput {
        id: None,
        name: "Paracetamol 500 mg".into(),
        generic_name: Some("Paracetamol".into()),
        manufacturer_id: None,
        category_id: None,
        drug_class: DrugClass::Free,
        is_owa: false,
        base_unit_id: TABLET,
        min_stock_base: 100,
        rack_id: None,
        units: vec![
            unit(TABLET, 1, false, &[]),
            unit(STRIP, 10, true, &["8991234567890"]),
            unit(BOX, 100, false, &[]),
        ],
    }
}

fn unit_id_of(p: &ProductDetail, unit_id: i64) -> i64 {
    p.units.iter().find(|u| u.unit_id == unit_id && u.is_active).unwrap().id
}

fn prices(p: &ProductDetail, cost: Option<i64>, margin: Option<i64>) -> ProductPricesInput {
    ProductPricesInput {
        product_id: p.id,
        margin_bp: margin,
        last_cost_x100: cost,
        units: p
            .units
            .iter()
            .filter(|u| u.is_active)
            .map(|u| UnitPriceInput {
                product_unit_id: u.id,
                price_mode: PriceMode::Auto,
                sell_price: 0,
                tiers: vec![],
            })
            .collect(),
    }
}

fn price_of(p: &ProductDetail, unit_id: i64) -> i64 {
    p.units.iter().find(|u| u.unit_id == unit_id && u.is_active).unwrap().sell_price
}

#[test]
fn create_product_with_units_and_search() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let saved = service::save_product(&mut conn, &owner, &paracetamol()).unwrap();
    let p = saved.product;

    assert_eq!(p.code, "OBT00001");
    assert_eq!(p.units.len(), 3);
    assert!(saved.warnings.iter().any(|w| w.contains("harganya masih 0")));

    let search = |q: &str| {
        service::list_products(
            &conn,
            &ProductListQuery { q: Some(q.into()), limit: 50, ..Default::default() },
        )
        .unwrap()
    };
    assert_eq!(search("para 500").total, 1);
    assert_eq!(search("parac").rows[0].sale_unit_name.as_deref(), Some("Strip"));
    assert_eq!(search("8991234567890").total, 1, "barcode persis");
    assert_eq!(search("amoxicillin").total, 0);
    assert_eq!(search("---").total, 0);
}

#[test]
fn rejects_invalid_units() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);

    let mut no_base = paracetamol();
    no_base.units.remove(0);
    assert!(matches!(service::save_product(&mut conn, &owner, &no_base), Err(AppError::Validation(_))));

    let mut same_conversion = paracetamol();
    same_conversion.units[2].conversion = 10;
    assert!(matches!(service::save_product(&mut conn, &owner, &same_conversion), Err(AppError::Validation(_))));

    let mut two_defaults = paracetamol();
    two_defaults.units[0].is_default_sale = true;
    assert!(matches!(service::save_product(&mut conn, &owner, &two_defaults), Err(AppError::Validation(_))));

    let mut owa_free = paracetamol();
    owa_free.is_owa = true;
    assert!(matches!(service::save_product(&mut conn, &owner, &owa_free), Err(AppError::Validation(_))));
}

#[test]
fn barcode_must_be_unique_across_products() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    service::save_product(&mut conn, &owner, &paracetamol()).unwrap();

    let mut other = paracetamol();
    other.name = "Paracetamol Sirup".into();
    let err = service::save_product(&mut conn, &owner, &other).unwrap_err();
    assert!(matches!(err, AppError::Conflict(m) if m.contains("Paracetamol 500 mg")));
}

#[test]
fn auto_price_follows_margin_chain() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let p = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;

    // HPP Rp150,50/tablet, margin default 20%, dibulatkan ke Rp100.
    let p = service::save_prices(&mut conn, &owner, &prices(&p, Some(15_050), None)).unwrap().product;
    assert_eq!(p.effective_margin_bp, Some(2_000));
    assert_eq!(price_of(&p, TABLET), 200); // 180,6 → 200
    assert_eq!(price_of(&p, STRIP), 1_900); // 1806 → 1900
    assert_eq!(price_of(&p, BOX), 18_100); // 18060 → 18100

    // Margin kategori 50% berlaku bila obat tidak punya margin sendiri.
    let cat = service::save_category(
        &mut conn,
        &owner,
        &CategoryInput { id: None, name: "Analgesik".into(), margin_bp: Some(5_000), is_active: true },
    )
    .unwrap();
    let mut input = paracetamol();
    input.id = Some(p.id);
    input.category_id = Some(cat.id);
    input.units = p
        .units
        .iter()
        .map(|u| ProductUnitInput {
            id: Some(u.id),
            unit_id: u.unit_id,
            conversion: u.conversion,
            is_default_sale: u.is_default_sale,
            barcodes: u.barcodes.clone(),
        })
        .collect();
    let p = service::save_product(&mut conn, &owner, &input).unwrap().product;
    assert_eq!(price_of(&p, STRIP), 2_300); // 2257,5 → 2300

    // Mengubah margin kategori menghitung ulang harga obat di kategori itu.
    service::save_category(
        &mut conn,
        &owner,
        &CategoryInput { id: Some(cat.id), name: "Analgesik".into(), margin_bp: Some(1_000), is_active: true },
    )
    .unwrap();
    let p = service::get_product(&conn, p.id, PriceAccess::of(&owner)).unwrap();
    assert_eq!(price_of(&p, STRIP), 1_700); // 1655,5 → 1700

    // Margin milik obat mengalahkan kategori.
    let p = service::save_prices(&mut conn, &owner, &prices(&p, Some(15_050), Some(0))).unwrap().product;
    assert_eq!(price_of(&p, STRIP), 1_600); // 1505 → 1600
}

#[test]
fn tiers_must_get_cheaper_and_warn_below_cost() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let p = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;
    let strip = unit_id_of(&p, STRIP);

    let mut input = prices(&p, Some(15_050), None);
    let strip_price = input.units.iter_mut().find(|u| u.product_unit_id == strip).unwrap();
    strip_price.price_mode = PriceMode::Manual;
    strip_price.sell_price = 2_000;
    strip_price.tiers = vec![
        PriceTierInput { min_qty: 20, price_mode: PriceMode::Manual, margin_bp: None, price: 1_500 },
        PriceTierInput { min_qty: 5, price_mode: PriceMode::Auto, margin_bp: Some(1_000), price: 0 },
    ];
    let saved = service::save_prices(&mut conn, &owner, &input).unwrap();
    let tiers = &saved.product.units.iter().find(|u| u.id == strip).unwrap().tiers;
    assert_eq!(tiers.iter().map(|t| (t.min_qty, t.price)).collect::<Vec<_>>(), vec![(5, 1_700), (20, 1_500)]);
    assert!(saved.warnings.iter().any(|w| w.contains("tier Strip ≥ 20 di bawah HPP")));

    // Tier yang tidak lebih murah ditolak.
    let strip_price = input.units.iter_mut().find(|u| u.product_unit_id == strip).unwrap();
    strip_price.tiers[0].price = 1_800;
    assert!(matches!(service::save_prices(&mut conn, &owner, &input), Err(AppError::Validation(_))));

    // Menghapus tier (tidak dikirim) menonaktifkannya.
    let strip_price = input.units.iter_mut().find(|u| u.product_unit_id == strip).unwrap();
    strip_price.tiers.clear();
    let saved = service::save_prices(&mut conn, &owner, &input).unwrap();
    assert!(saved.product.units.iter().find(|u| u.id == strip).unwrap().tiers.is_empty());
}

#[test]
fn technician_does_not_see_cost_or_margin() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let p = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;
    service::save_prices(&mut conn, &owner, &prices(&p, Some(15_050), Some(3_000))).unwrap();

    let ttk = user(&[Role::Technician]);
    let seen = service::get_product(&conn, p.id, PriceAccess::of(&ttk)).unwrap();
    assert_eq!(seen.last_cost_x100, None);
    assert_eq!(seen.margin_bp, None);
    assert_eq!(seen.effective_margin_bp, None);
    assert!(seen.units.iter().all(|u| u.sell_price > 0), "harga jual tetap terlihat");

    // TTK mengubah data obat tidak menghapus HPP/margin.
    let mut input = paracetamol();
    input.id = Some(p.id);
    input.units = vec![unit(TABLET, 1, false, &[]), unit(STRIP, 10, true, &["8991234567890"])];
    service::save_product(&mut conn, &ttk, &input).unwrap();
    let after = service::get_product(&conn, p.id, PriceAccess::of(&owner)).unwrap();
    assert_eq!(after.last_cost_x100, Some(15_050));
    assert_eq!(after.margin_bp, Some(3_000));
}

#[test]
fn stock_locks_base_unit_and_conversions() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let p = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;
    conn.execute(
        "INSERT INTO batches (product_id, batch_number, expiry_date, unit_cost_x100, source_type)
         VALUES (?1, 'B1', '2030-01-01', 15050, 'OPENING')",
        [p.id],
    )
    .unwrap();

    let existing = |p: &ProductDetail| -> Vec<ProductUnitInput> {
        p.units
            .iter()
            .filter(|u| u.is_active)
            .map(|u| ProductUnitInput {
                id: Some(u.id),
                unit_id: u.unit_id,
                conversion: u.conversion,
                is_default_sale: u.is_default_sale,
                barcodes: u.barcodes.clone(),
            })
            .collect()
    };

    let mut change_conversion = paracetamol();
    change_conversion.id = Some(p.id);
    change_conversion.units = existing(&p);
    change_conversion.units.iter_mut().find(|u| u.unit_id == BOX).unwrap().conversion = 200;
    assert!(matches!(service::save_product(&mut conn, &owner, &change_conversion), Err(AppError::Validation(_))));

    let mut change_base = paracetamol();
    change_base.id = Some(p.id);
    change_base.base_unit_id = STRIP;
    change_base.units = vec![unit(STRIP, 1, true, &[])];
    assert!(matches!(service::save_product(&mut conn, &owner, &change_base), Err(AppError::Validation(_))));

    // Menambah satuan baru tetap boleh.
    let mut add_unit = paracetamol();
    add_unit.id = Some(p.id);
    add_unit.units = existing(&p);
    add_unit.units.push(unit(7, 50, false, &[])); // Botol isi 50
    let p = service::save_product(&mut conn, &owner, &add_unit).unwrap().product;
    assert_eq!(p.units.iter().filter(|u| u.is_active).count(), 4);
}

#[test]
fn removed_unit_is_deactivated_and_can_come_back() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let p = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;
    let box_id = unit_id_of(&p, BOX);

    let mut without_box = paracetamol();
    without_box.id = Some(p.id);
    without_box.units.remove(2);
    let p = service::save_product(&mut conn, &owner, &without_box).unwrap().product;
    assert!(!p.units.iter().find(|u| u.id == box_id).unwrap().is_active);

    let mut with_box = paracetamol();
    with_box.id = Some(p.id);
    let p = service::save_product(&mut conn, &owner, &with_box).unwrap().product;
    let again = p.units.iter().find(|u| u.unit_id == BOX).unwrap();
    assert_eq!(again.id, box_id);
    assert!(again.is_active);
}

#[test]
fn category_margin_requires_price_permission() {
    let mut conn = setup();
    let ttk = user(&[Role::Technician]);
    let err = service::save_category(
        &mut conn,
        &ttk,
        &CategoryInput { id: None, name: "Vitamin".into(), margin_bp: Some(3_000), is_active: true },
    );
    assert!(matches!(err, Err(AppError::Forbidden)));

    let cat = service::save_category(
        &mut conn,
        &ttk,
        &CategoryInput { id: None, name: "Vitamin".into(), margin_bp: None, is_active: true },
    )
    .unwrap();
    assert_eq!(cat.margin_bp, None);
    assert_eq!(cat.created_by.as_deref(), Some("u"));
    assert!(!cat.created_at.is_empty());

    let dup = service::save_category(
        &mut conn,
        &ttk,
        &CategoryInput { id: None, name: "vitamin".into(), margin_bp: None, is_active: true },
    );
    assert!(matches!(dup, Err(AppError::Conflict(_))));
}

#[test]
fn racks_and_manufacturers_are_master_data() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let named = |id: Option<i64>, name: &str, active: bool| NamedItemInput { id, name: name.into(), is_active: active };

    let rack = service::save_named(&conn, &owner, NamedTable::Racks, &named(None, "A1", true)).unwrap();
    assert_eq!(rack.created_by.as_deref(), Some("u"));
    assert!(!rack.created_at.is_empty());
    let pabrik = service::save_named(&conn, &owner, NamedTable::Manufacturers, &named(None, "Kimia Farma", true)).unwrap();

    // Nama unik tanpa membedakan huruf besar/kecil, per tabel.
    let dup = service::save_named(&conn, &owner, NamedTable::Racks, &named(None, "a1", true));
    assert!(matches!(dup, Err(AppError::Conflict(_))));
    service::save_named(&conn, &owner, NamedTable::Manufacturers, &named(None, "A1", true)).unwrap();

    let mut input = paracetamol();
    input.rack_id = Some(rack.id);
    input.manufacturer_id = Some(pabrik.id);
    let p = service::save_product(&mut conn, &owner, &input).unwrap().product;
    assert_eq!((p.rack_id, p.manufacturer_id), (Some(rack.id), Some(pabrik.id)));

    let rows = service::list_products(&conn, &ProductListQuery { limit: 10, ..Default::default() }).unwrap().rows;
    assert_eq!(rows[0].rack_name.as_deref(), Some("A1"));

    // Ganti nama rak ikut tampil di daftar obat.
    service::save_named(&conn, &owner, NamedTable::Racks, &named(Some(rack.id), "A1-Atas", true)).unwrap();
    let rows = service::list_products(&conn, &ProductListQuery { limit: 10, ..Default::default() }).unwrap().rows;
    assert_eq!(rows[0].rack_name.as_deref(), Some("A1-Atas"));

    let mut bad = paracetamol();
    bad.name = "Obat lain".into();
    bad.units[1].barcodes.clear();
    bad.rack_id = Some(999);
    assert!(matches!(service::save_product(&mut conn, &owner, &bad), Err(AppError::Validation(_))));

    let missing = service::save_named(&conn, &owner, NamedTable::Racks, &named(Some(999), "X", true));
    assert!(matches!(missing, Err(AppError::NotFound(_))));
}

#[test]
fn master_codes_are_always_system_generated_and_immutable() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let named = |id: Option<i64>, name: &str| NamedItemInput { id, name: name.into(), is_active: true };

    // Kode otomatis per jenis master, berurutan.
    let a1 = service::save_named(&conn, &owner, NamedTable::Racks, &named(None, "A1")).unwrap();
    let b1 = service::save_named(&conn, &owner, NamedTable::Racks, &named(None, "B1")).unwrap();
    assert_eq!((a1.code.as_str(), b1.code.as_str()), ("RAK0001", "RAK0002"));
    let pabrik = service::save_named(&conn, &owner, NamedTable::Manufacturers, &named(None, "Kalbe")).unwrap();
    assert_eq!(pabrik.code, "PBR0001");
    let cat = service::save_category(
        &mut conn,
        &owner,
        &CategoryInput { id: None, name: "Vitamin".into(), margin_bp: None, is_active: true },
    )
    .unwrap();
    assert_eq!(cat.code, "KTG0001");

    // Mengubah data tidak mengubah kode.
    let renamed = service::save_named(&conn, &owner, NamedTable::Racks, &named(Some(a1.id), "A1 Atas")).unwrap();
    assert_eq!(renamed.code, "RAK0001");
    let cat2 = service::save_category(
        &mut conn,
        &owner,
        &CategoryInput { id: Some(cat.id), name: "Vitamin & Suplemen".into(), margin_bp: None, is_active: false },
    )
    .unwrap();
    assert_eq!(cat2.code, "KTG0001");

    // Kode tidak bisa diubah, bahkan langsung lewat SQL.
    for (table, id) in [("racks", a1.id), ("manufacturers", pabrik.id), ("categories", cat.id)] {
        let changed = conn.execute(&format!("UPDATE {table} SET code = 'MANUAL-1' WHERE id = ?1"), [id]);
        assert!(changed.is_err(), "{table}");
    }
}

#[test]
fn product_code_is_system_generated_and_immutable() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let first = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;
    let mut other = paracetamol();
    other.name = "Amoxicillin 500 mg".into();
    other.units = vec![unit(TABLET, 1, true, &[])];
    let second = service::save_product(&mut conn, &owner, &other).unwrap().product;
    assert_eq!((first.code.as_str(), second.code.as_str()), ("OBT00001", "OBT00002"));

    // Mengubah data tidak mengubah kode.
    let mut edit = paracetamol();
    edit.id = Some(first.id);
    edit.name = "Paracetamol 500 mg Strip".into();
    edit.units = first
        .units
        .iter()
        .map(|u| ProductUnitInput {
            id: Some(u.id),
            unit_id: u.unit_id,
            conversion: u.conversion,
            is_default_sale: u.is_default_sale,
            barcodes: u.barcodes.clone(),
        })
        .collect();
    let edited = service::save_product(&mut conn, &owner, &edit).unwrap().product;
    assert_eq!(edited.code, "OBT00001");

    // Kode tidak bisa diubah, bahkan langsung lewat SQL.
    assert!(conn.execute("UPDATE products SET code = 'MANUAL-1' WHERE id = ?1", [first.id]).is_err());
}

#[test]
fn master_data_is_soft_deleted_only_when_unused() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let named = |name: &str| NamedItemInput { id: None, name: name.into(), is_active: true };

    let used_rack = service::save_named(&conn, &owner, NamedTable::Racks, &named("A1")).unwrap();
    let free_rack = service::save_named(&conn, &owner, NamedTable::Racks, &named("B1")).unwrap();
    let cat = service::save_category(
        &mut conn,
        &owner,
        &CategoryInput { id: None, name: "Vitamin".into(), margin_bp: None, is_active: true },
    )
    .unwrap();
    let mut input = paracetamol();
    input.rack_id = Some(used_rack.id);
    input.category_id = Some(cat.id);
    service::save_product(&mut conn, &owner, &input).unwrap();

    // Nonaktif / aktif.
    service::set_master_active(&conn, &owner, MasterKind::Rack, used_rack.id, false).unwrap();
    let racks = service::list_named(&conn, NamedTable::Racks).unwrap();
    assert!(!racks.iter().find(|r| r.id == used_rack.id).unwrap().is_active);
    service::set_master_active(&conn, &owner, MasterKind::Rack, used_rack.id, true).unwrap();

    // Yang dipakai obat tidak bisa dihapus.
    let err = service::delete_master(&mut conn, &owner, MasterKind::Rack, used_rack.id).unwrap_err();
    assert!(matches!(err, AppError::Conflict(m) if m.contains("1 obat")));
    let err = service::delete_master(&mut conn, &owner, MasterKind::Category, cat.id).unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)));

    // Yang belum dipakai bisa dihapus: soft delete, tersembunyi tapi tetap ada di database.
    service::delete_master(&mut conn, &owner, MasterKind::Rack, free_rack.id).unwrap();
    let racks = service::list_named(&conn, NamedTable::Racks).unwrap();
    assert_eq!(racks.len(), 1);
    let (deleted_at, deleted_by): (Option<String>, Option<i64>) = conn
        .query_row("SELECT deleted_at, deleted_by FROM racks WHERE id = ?1", [free_rack.id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert!(deleted_at.is_some());
    assert_eq!(deleted_by, Some(owner.id));

    // Data terhapus tidak bisa diubah, diaktifkan, atau dihapus lagi.
    let edit = service::save_named(
        &conn,
        &owner,
        NamedTable::Racks,
        &NamedItemInput { id: Some(free_rack.id), name: "B1".into(), is_active: true },
    );
    assert!(matches!(edit, Err(AppError::NotFound(_))));
    let activate = service::set_master_active(&conn, &owner, MasterKind::Rack, free_rack.id, true);
    assert!(matches!(activate, Err(AppError::NotFound(_))));
    let again = service::delete_master(&mut conn, &owner, MasterKind::Rack, free_rack.id);
    assert!(matches!(again, Err(AppError::NotFound(_))));

    // Nama data terhapus boleh dipakai lagi, tapi kodenya tidak dipakai ulang.
    let reused = service::save_named(&conn, &owner, NamedTable::Racks, &named("B1")).unwrap();
    assert_ne!(reused.code, free_rack.code);

    // Hapus permanen selalu ditolak oleh database.
    assert!(conn.execute("DELETE FROM racks WHERE id = ?1", [free_rack.id]).is_err());

    let missing = service::delete_master(&mut conn, &owner, MasterKind::Manufacturer, 999);
    assert!(matches!(missing, Err(AppError::NotFound(_))));
    let missing = service::set_master_active(&conn, &owner, MasterKind::Rack, 999, false);
    assert!(matches!(missing, Err(AppError::NotFound(_))));
}

#[test]
fn master_pages_are_paginated_and_searchable() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    for i in 1..=30 {
        service::save_named(&conn, &owner, NamedTable::Racks, &NamedItemInput { id: None, name: format!("Rak {i:02}"), is_active: true })
            .unwrap();
    }
    service::save_named(&conn, &owner, NamedTable::Racks, &NamedItemInput { id: None, name: "Diskon 50%_A".into(), is_active: true })
        .unwrap();
    let page = |conn: &Connection, q: Option<&str>, offset: i64, limit: i64| {
        service::page_named(conn, NamedTable::Racks, &MasterPageQuery { q: q.map(Into::into), offset, limit }).unwrap()
    };

    // Halaman: urut nama, total semua data.
    let first = page(&conn, None, 0, 25);
    assert_eq!((first.rows.len(), first.total), (25, 31));
    let second = page(&conn, None, 25, 25);
    assert_eq!(second.rows.len(), 6);
    assert!(first.rows.iter().all(|r| !second.rows.iter().any(|s| s.id == r.id)));

    // Cari nama atau kode, tanpa beda huruf besar/kecil.
    assert_eq!(page(&conn, Some("rak 0"), 0, 25).total, 9);
    assert_eq!(page(&conn, Some("rak0031"), 0, 25).rows[0].name, "Diskon 50%_A");
    // % dan _ dicari apa adanya, bukan wildcard.
    assert_eq!(page(&conn, Some("50%_"), 0, 25).total, 1);
    assert_eq!(page(&conn, Some("_"), 0, 25).total, 1);

    // Data terhapus tidak ikut.
    let gone = page(&conn, Some("Rak 30"), 0, 25).rows[0].id;
    service::delete_master(&mut conn, &owner, MasterKind::Rack, gone).unwrap();
    assert_eq!(page(&conn, None, 0, 100).total, 30);

    // Margin kategori disembunyikan dari TTK.
    service::save_category(
        &mut conn,
        &owner,
        &CategoryInput { id: None, name: "Vitamin".into(), margin_bp: Some(3_000), is_active: true },
    )
    .unwrap();
    let q = MasterPageQuery { q: Some("vit".into()), offset: 0, limit: 10 };
    let for_owner = service::page_categories(&conn, PriceAccess::of(&owner), &q).unwrap();
    assert_eq!(for_owner.rows[0].margin_bp, Some(3_000));
    assert_eq!(for_owner.default_margin_bp, Some(2_000));
    let for_ttk = service::page_categories(&conn, PriceAccess::of(&user(&[Role::Technician])), &q).unwrap();
    assert_eq!(for_ttk.rows[0].margin_bp, None);
    assert_eq!(for_ttk.default_margin_bp, None);
}

#[test]
fn barcodes_are_soft_deleted_and_unchanged_ones_kept() {
    let mut conn = setup();
    let owner = user(&[Role::Owner]);
    let p = service::save_product(&mut conn, &owner, &paracetamol()).unwrap().product;
    let original_row: i64 = conn
        .query_row("SELECT id FROM product_barcodes WHERE barcode = '8991234567890'", [], |r| r.get(0))
        .unwrap();

    let resave = |conn: &mut Connection, strip: &[&str], boxes: &[&str]| {
        let mut input = paracetamol();
        input.id = Some(p.id);
        input.units = vec![unit(TABLET, 1, false, &[]), unit(STRIP, 10, true, strip), unit(BOX, 100, false, boxes)];
        service::save_product(conn, &owner, &input).unwrap().product
    };

    // Simpan ulang tanpa perubahan: barcode tetap baris yang sama (tidak dihapus-tambah ulang).
    resave(&mut conn, &["8991234567890"], &[]);
    let same_row: i64 = conn
        .query_row("SELECT id FROM product_barcodes WHERE barcode = '8991234567890' AND deleted_at IS NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(same_row, original_row);

    // Barcode dilepas: soft delete, tidak muncul di obat, pencarian, maupun pemilik barcode.
    let after = resave(&mut conn, &[], &["8990000000001"]);
    assert!(after.units.iter().all(|u| !u.barcodes.contains(&"8991234567890".to_string())));
    let (deleted_at, deleted_by): (Option<String>, Option<i64>) = conn
        .query_row("SELECT deleted_at, deleted_by FROM product_barcodes WHERE id = ?1", [original_row], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert!(deleted_at.is_some());
    assert_eq!(deleted_by, Some(owner.id));
    let search = service::list_products(&conn, &ProductListQuery { q: Some("8991234567890".into()), limit: 10, ..Default::default() })
        .unwrap();
    assert_eq!(search.total, 0);

    // Barcode yang sudah dilepas boleh dipakai obat lain.
    let mut other = paracetamol();
    other.name = "Paracetamol Sirup".into();
    service::save_product(&mut conn, &owner, &other).unwrap();

    // Barcode boleh dipindah ke satuan lain di obat yang sama.
    let moved = resave(&mut conn, &["8990000000001"], &[]);
    let strip = moved.units.iter().find(|u| u.unit_id == STRIP).unwrap();
    assert_eq!(strip.barcodes, vec!["8990000000001".to_string()]);

    // Hapus permanen ditolak oleh database.
    assert!(conn.execute("DELETE FROM product_barcodes", []).is_err());
}
