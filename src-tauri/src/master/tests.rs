use rusqlite::Connection;

use super::model::*;
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
        code: None,
        name: "Paracetamol 500 mg".into(),
        generic_name: Some("Paracetamol".into()),
        manufacturer: Some("Kimia Farma".into()),
        category_id: None,
        drug_class: DrugClass::Free,
        is_owa: false,
        base_unit_id: TABLET,
        min_stock_base: 100,
        rack_location: Some("A1".into()),
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
    assert_eq!(search("kimia").total, 0, "pabrik tidak ikut dicari");
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

    let dup = service::save_category(
        &mut conn,
        &ttk,
        &CategoryInput { id: None, name: "vitamin".into(), margin_bp: None, is_active: true },
    );
    assert!(matches!(dup, Err(AppError::Conflict(_))));
}
