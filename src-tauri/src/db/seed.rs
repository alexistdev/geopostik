//! Data awal master (kategori, rak, pabrik, obat) untuk database yang baru dibuat.
//!
//! Merek dan pabrik diambil dari obat yang beredar di Indonesia. HPP hanya perkiraan dan harga
//! jual dihitung otomatis dari HPP + margin, jadi keduanya perlu disesuaikan oleh apotek.

use rusqlite::{Connection, params};

use crate::error::{AppError, AppResult};
use crate::master::pricing::auto_price;
use crate::settings;

/// (nama, margin dalam basis point; `None` = pakai margin default)
const CATEGORIES: [(&str, Option<i64>); 5] = [
    ("Analgesik & Antipiretik", None),
    ("Antibiotik & Antiinfeksi", Some(1_500)),
    ("Batuk, Flu & Alergi", None),
    ("Saluran Cerna", None),
    ("Vitamin & Suplemen", Some(2_500)),
];

const RACKS: [&str; 10] = [
    "Etalase Depan 1",
    "Etalase Depan 2",
    "Rak Obat Bebas A",
    "Rak Obat Bebas B",
    "Rak Obat Keras A",
    "Rak Obat Keras B",
    "Rak Sirup",
    "Rak Salep & Tetes",
    "Lemari Pendingin",
    "Gudang",
];

const MANUFACTURERS: [&str; 20] = [
    "Kimia Farma",
    "Kalbe Farma",
    "Sanbe Farma",
    "Dexa Medica",
    "Tempo Scan Pacific",
    "Konimex",
    "Sido Muncul",
    "Bayer Indonesia",
    "Pfizer Indonesia",
    "Phapros",
    "Indofarma",
    "Novell Pharmaceutical Laboratories",
    "Hexpharm Jaya",
    "Medifarma Laboratories",
    "Combiphar",
    "Soho Global Health",
    "Interbat",
    "Darya-Varia Laboratoria",
    "Pharos Indonesia",
    "Haleon Indonesia",
];

// Indeks kategori.
const ANALGESIK: usize = 0;
const ANTIBIOTIK: usize = 1;
const BATUK: usize = 2;
const CERNA: usize = 3;
const VITAMIN: usize = 4;

// Indeks rak.
const ETALASE_1: usize = 0;
const ETALASE_2: usize = 1;
const BEBAS_A: usize = 2;
const BEBAS_B: usize = 3;
const KERAS_A: usize = 4;
const KERAS_B: usize = 5;
const SIRUP: usize = 6;
const SALEP: usize = 7;

struct SeedProduct {
    name: &'static str,
    generic: &'static str,
    category: usize,
    manufacturer: &'static str,
    rack: usize,
    drug_class: &'static str,
    is_owa: bool,
    /// Satuan dasar ada di urutan pertama dengan isi 1.
    units: &'static [(&'static str, i64)],
    default_sale: &'static str,
    /// HPP per satuan dasar, rupiah.
    cost: i64,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    name: &'static str,
    generic: &'static str,
    category: usize,
    manufacturer: &'static str,
    rack: usize,
    drug_class: &'static str,
    units: &'static [(&'static str, i64)],
    default_sale: &'static str,
    cost: i64,
) -> SeedProduct {
    SeedProduct { name, generic, category, manufacturer, rack, drug_class, is_owa: false, units, default_sale, cost }
}

const fn owa(mut product: SeedProduct) -> SeedProduct {
    product.is_owa = true;
    product
}

const TAB_10_100: &[(&str, i64)] = &[("Tablet", 1), ("Strip", 10), ("Box", 100)];
const KPL_10_100: &[(&str, i64)] = &[("Kaplet", 1), ("Strip", 10), ("Box", 100)];
const KPS_10_100: &[(&str, i64)] = &[("Kapsul", 1), ("Strip", 10), ("Box", 100)];
const KPS_10_30: &[(&str, i64)] = &[("Kapsul", 1), ("Strip", 10), ("Box", 30)];
const TAB_4_100: &[(&str, i64)] = &[("Tablet", 1), ("Strip", 4), ("Box", 100)];
const BOTOL: &[(&str, i64)] = &[("Botol", 1)];

const PRODUCTS: [SeedProduct; 50] = [
    // ─── Analgesik & Antipiretik ─────────────────────────────────────────────
    p("Panadol 500 mg", "Paracetamol", ANALGESIK, "Haleon Indonesia", ETALASE_1, "FREE", KPL_10_100, "Strip", 1_000),
    p("Sanmol 500 mg", "Paracetamol", ANALGESIK, "Sanbe Farma", ETALASE_1, "FREE", TAB_10_100, "Strip", 400),
    p("Paracetamol 500 mg", "Paracetamol", ANALGESIK, "Kimia Farma", ETALASE_1, "FREE", TAB_10_100, "Strip", 150),
    p("Bodrex", "Paracetamol, Kafein", ANALGESIK, "Tempo Scan Pacific", ETALASE_1, "FREE",
      &[("Tablet", 1), ("Strip", 10), ("Box", 200)], "Strip", 350),
    p("Paramex", "Paracetamol, Propifenazon, Kafein, Deksklorfeniramin Maleat", ANALGESIK, "Konimex", ETALASE_1,
      "LIMITED_FREE", &[("Tablet", 1), ("Strip", 4), ("Box", 80)], "Strip", 700),
    p("Ponstan 500 mg", "Asam Mefenamat", ANALGESIK, "Pfizer Indonesia", KERAS_B, "HARD", TAB_10_100, "Strip", 2_500),
    owa(p("Asam Mefenamat 500 mg", "Asam Mefenamat", ANALGESIK, "Hexpharm Jaya", KERAS_B, "HARD", TAB_10_100, "Strip", 250)),
    p("Voltadex 50 mg", "Natrium Diklofenak", ANALGESIK, "Dexa Medica", KERAS_B, "HARD",
      &[("Tablet", 1), ("Strip", 10), ("Box", 50)], "Strip", 900),
    p("Proris Suspensi 60 ml", "Ibuprofen", ANALGESIK, "Pharos Indonesia", SIRUP, "LIMITED_FREE", BOTOL, "Botol", 22_000),
    p("Termorex Sirup 60 ml", "Paracetamol", ANALGESIK, "Konimex", SIRUP, "FREE", BOTOL, "Botol", 12_000),
    // ─── Antibiotik & Antiinfeksi ────────────────────────────────────────────
    p("Amoxicillin 500 mg", "Amoksisilin", ANTIBIOTIK, "Indofarma", KERAS_A, "HARD", KPS_10_100, "Strip", 350),
    p("Amoxsan 500 mg", "Amoksisilin", ANTIBIOTIK, "Sanbe Farma", KERAS_A, "HARD", KPS_10_100, "Strip", 2_200),
    p("Kalmoxillin 500 mg", "Amoksisilin", ANTIBIOTIK, "Kalbe Farma", KERAS_A, "HARD", KPL_10_100, "Strip", 1_800),
    p("Cefadroxil 500 mg", "Sefadroksil", ANTIBIOTIK, "Hexpharm Jaya", KERAS_A, "HARD", KPS_10_100, "Strip", 900),
    p("Cefixime 100 mg", "Sefiksim", ANTIBIOTIK, "Novell Pharmaceutical Laboratories", KERAS_A, "HARD", KPS_10_30,
      "Strip", 2_000),
    p("Ciprofloxacin 500 mg", "Siprofloksasin", ANTIBIOTIK, "Kimia Farma", KERAS_A, "HARD", TAB_10_100, "Strip", 400),
    p("Metronidazole 500 mg", "Metronidazol", ANTIBIOTIK, "Phapros", KERAS_A, "HARD", TAB_10_100, "Strip", 250),
    p("Azithromycin 500 mg", "Azitromisin", ANTIBIOTIK, "Hexpharm Jaya", KERAS_A, "HARD",
      &[("Tablet", 1), ("Strip", 3)], "Strip", 3_500),
    p("Acyclovir 400 mg", "Asiklovir", ANTIBIOTIK, "Kimia Farma", KERAS_A, "HARD", TAB_10_100, "Strip", 450),
    p("Kalpanax Krim 5 g", "Mikonazol Nitrat 2%", ANTIBIOTIK, "Kalbe Farma", SALEP, "LIMITED_FREE", &[("Tube", 1)],
      "Tube", 9_000),
    // ─── Batuk, Flu & Alergi ─────────────────────────────────────────────────
    p("OBH Combi Batuk Berdahak 100 ml", "Succus Liquiritiae, Amonium Klorida", BATUK, "Combiphar", SIRUP, "FREE",
      BOTOL, "Botol", 16_000),
    p("Siladex Batuk & Pilek 60 ml", "Dekstrometorfan, Pseudoefedrin, Klorfeniramin", BATUK, "Konimex", SIRUP,
      "LIMITED_FREE", BOTOL, "Botol", 14_000),
    p("Mixagrip Flu & Batuk", "Paracetamol, Fenilefrin, Dekstrometorfan, Klorfeniramin", BATUK, "Kalbe Farma",
      ETALASE_2, "LIMITED_FREE", &[("Kaplet", 1), ("Strip", 4), ("Box", 100)], "Strip", 650),
    p("Decolgen", "Paracetamol, Fenilefrin, Klorfeniramin Maleat", BATUK, "Medifarma Laboratories", ETALASE_2,
      "LIMITED_FREE", TAB_4_100, "Strip", 600),
    p("Neozep Forte", "Paracetamol, Fenilefrin, Klorfeniramin Maleat", BATUK, "Medifarma Laboratories", ETALASE_2,
      "LIMITED_FREE", TAB_4_100, "Strip", 650),
    p("Bodrex Flu & Batuk PE", "Paracetamol, Fenilefrin, Dekstrometorfan", BATUK, "Tempo Scan Pacific", ETALASE_2,
      "LIMITED_FREE", &[("Kaplet", 1), ("Strip", 4), ("Box", 80)], "Strip", 700),
    p("Sanadryl DMP Sirup 60 ml", "Difenhidramin, Dekstrometorfan", BATUK, "Sanbe Farma", SIRUP, "LIMITED_FREE", BOTOL,
      "Botol", 13_000),
    p("Incidal-OD 10 mg", "Setirizin", BATUK, "Bayer Indonesia", KERAS_B, "HARD",
      &[("Kapsul", 1), ("Strip", 10), ("Box", 50)], "Strip", 4_500),
    p("Interhistin 50 mg", "Mebhidrolin Napadisilat", BATUK, "Interbat", ETALASE_2, "LIMITED_FREE", TAB_10_100,
      "Strip", 700),
    p("Ambroxol 30 mg", "Ambroksol HCl", BATUK, "Indofarma", KERAS_B, "HARD", TAB_10_100, "Strip", 200),
    // ─── Saluran Cerna ───────────────────────────────────────────────────────
    p("Promag", "Hidrotalsit, Magnesium Hidroksida, Simetikon", CERNA, "Kalbe Farma", ETALASE_2, "FREE",
      &[("Tablet", 1), ("Strip", 12), ("Box", 144)], "Strip", 600),
    p("Antasida DOEN", "Aluminium Hidroksida, Magnesium Hidroksida", CERNA, "Kimia Farma", BEBAS_B, "FREE", TAB_10_100,
      "Strip", 150),
    p("Plantacid Forte Suspensi 100 ml", "Aluminium Hidroksida, Magnesium Hidroksida, Simetikon", CERNA,
      "Novell Pharmaceutical Laboratories", SIRUP, "FREE", BOTOL, "Botol", 14_000),
    p("Oralit 200", "Natrium Klorida, Kalium Klorida, Natrium Bikarbonat, Glukosa", CERNA, "Kimia Farma", BEBAS_B,
      "FREE", &[("Sachet", 1)], "Sachet", 800),
    p("Tolak Angin Cair 15 ml", "Herbal (Jahe, Adas, Kayu Ules, Daun Cengkeh, Madu)", CERNA, "Sido Muncul", ETALASE_2,
      "FREE", &[("Sachet", 1), ("Box", 12)], "Sachet", 3_000),
    p("Omeprazole 20 mg", "Omeprazol", CERNA, "Hexpharm Jaya", KERAS_B, "HARD", KPS_10_30, "Strip", 500),
    p("Lansoprazole 30 mg", "Lansoprazol", CERNA, "Dexa Medica", KERAS_B, "HARD", KPS_10_30, "Strip", 900),
    p("Domperidone 10 mg", "Domperidon", CERNA, "Indofarma", KERAS_B, "HARD", TAB_10_100, "Strip", 200),
    p("Antimo 50 mg", "Dimenhidrinat", CERNA, "Phapros", BEBAS_B, "LIMITED_FREE", TAB_10_100, "Strip", 800),
    p("Lacto-B", "Probiotik (Lactobacillus acidophilus, Bifidobacterium longum, Streptococcus thermophilus)", CERNA,
      "Novell Pharmaceutical Laboratories", BEBAS_B, "FREE", &[("Sachet", 1), ("Box", 10)], "Sachet", 4_500),
    // ─── Vitamin & Suplemen ──────────────────────────────────────────────────
    p("Enervon-C", "Multivitamin, Vitamin C", VITAMIN, "Medifarma Laboratories", BEBAS_A, "FREE",
      &[("Tablet", 1), ("Strip", 4), ("Box", 120)], "Strip", 1_000),
    p("Redoxon 1000 mg Effervescent", "Vitamin C (Asam Askorbat)", VITAMIN, "Bayer Indonesia", BEBAS_A, "FREE",
      &[("Tablet", 1), ("Tube", 10)], "Tube", 5_500),
    p("Imboost", "Ekstrak Echinacea purpurea, Zinc Picolinate", VITAMIN, "Soho Global Health", BEBAS_A, "FREE",
      &[("Tablet", 1), ("Strip", 10), ("Box", 30)], "Strip", 3_000),
    p("Curcuma Plus Sirup 60 ml", "Ekstrak Temulawak, Multivitamin", VITAMIN, "Soho Global Health", SIRUP, "FREE",
      BOTOL, "Botol", 13_000),
    p("Hemaviton Action", "Vitamin B Kompleks, Ginseng, Taurin", VITAMIN, "Tempo Scan Pacific", BEBAS_A, "FREE",
      &[("Tablet", 1), ("Strip", 5), ("Box", 50)], "Strip", 1_500),
    p("Sakatonik ABC", "Multivitamin", VITAMIN, "Konimex", BEBAS_A, "FREE", &[("Tablet", 1), ("Botol", 30)], "Botol",
      250),
    p("Stimuno Forte", "Ekstrak Phyllanthus niruri", VITAMIN, "Dexa Medica", BEBAS_A, "FREE", KPS_10_30, "Strip",
      3_000),
    p("Natur-E 100 IU", "Vitamin E (d-alpha Tocopherol)", VITAMIN, "Darya-Varia Laboratoria", BEBAS_A, "FREE",
      &[("Kapsul", 1), ("Botol", 32)], "Botol", 1_300),
    p("Vitamin B Kompleks", "Vitamin B1, B2, B6, Nikotinamid, Kalsium Pantotenat", VITAMIN, "Kimia Farma", BEBAS_A,
      "FREE", &[("Tablet", 1), ("Botol", 100)], "Botol", 100),
    p("Kalsium Laktat 500 mg", "Kalsium Laktat", VITAMIN, "Kimia Farma", BEBAS_A, "FREE", TAB_10_100, "Strip", 80),
];

/// Jumlah satuan jual utama yang dijadikan stok minimal.
const MIN_STOCK_SALE_UNITS: i64 = 5;

/// Mengisi data master awal. Dipanggil sekali untuk database yang baru dibuat; dilewati bila
/// master data sudah berisi agar data milik apotek tidak tercampur.
pub fn run(conn: &mut Connection) -> AppResult<bool> {
    let tx = conn.transaction()?;
    let existing: i64 = tx.query_row(
        "SELECT (SELECT count(*) FROM categories) + (SELECT count(*) FROM racks)
              + (SELECT count(*) FROM manufacturers) + (SELECT count(*) FROM products)",
        [],
        |r| r.get(0),
    )?;
    if existing > 0 {
        return Ok(false);
    }

    let mut category_ids = Vec::with_capacity(CATEGORIES.len());
    for (i, (name, margin_bp)) in CATEGORIES.iter().enumerate() {
        tx.execute(
            "INSERT INTO categories (code, name, margin_bp) VALUES (?1, ?2, ?3)",
            params![format!("KTG{:04}", i + 1), name, margin_bp],
        )?;
        category_ids.push(tx.last_insert_rowid());
    }

    let mut rack_ids = Vec::with_capacity(RACKS.len());
    for (i, name) in RACKS.iter().enumerate() {
        tx.execute("INSERT INTO racks (code, name) VALUES (?1, ?2)", params![format!("RAK{:04}", i + 1), name])?;
        rack_ids.push(tx.last_insert_rowid());
    }

    let mut manufacturer_ids = Vec::with_capacity(MANUFACTURERS.len());
    for (i, name) in MANUFACTURERS.iter().enumerate() {
        tx.execute(
            "INSERT INTO manufacturers (code, name) VALUES (?1, ?2)",
            params![format!("PBR{:04}", i + 1), name],
        )?;
        manufacturer_ids.push(tx.last_insert_rowid());
    }

    let price = settings::price(&tx)?;
    let unit_id = |name: &str| -> AppResult<i64> {
        tx.query_row("SELECT id FROM units WHERE name = ?1", [name], |r| r.get(0))
            .map_err(|_| AppError::Internal(format!("seed: satuan {name} tidak ada")))
    };

    for (i, sp) in PRODUCTS.iter().enumerate() {
        let manufacturer = MANUFACTURERS
            .iter()
            .position(|m| *m == sp.manufacturer)
            .ok_or_else(|| AppError::Internal(format!("seed: pabrik {} tidak ada", sp.manufacturer)))?;
        let (base_unit, _) = sp.units[0];
        let sale_conversion = sp
            .units
            .iter()
            .find(|(u, _)| *u == sp.default_sale)
            .map(|(_, c)| *c)
            .ok_or_else(|| AppError::Internal(format!("seed: satuan jual {} tidak ada", sp.name)))?;
        let cost_x100 = sp.cost * 100;
        let margin_bp = CATEGORIES[sp.category].1.unwrap_or(price.default_margin_bp);

        tx.execute(
            "INSERT INTO products (code, name, generic_name, manufacturer_id, category_id, drug_class, is_owa,
                                   base_unit_id, min_stock_base, rack_id, last_cost_x100)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                format!("OBT{:05}", i + 1),
                sp.name,
                sp.generic,
                manufacturer_ids[manufacturer],
                category_ids[sp.category],
                sp.drug_class,
                sp.is_owa,
                unit_id(base_unit)?,
                sale_conversion * MIN_STOCK_SALE_UNITS,
                rack_ids[sp.rack],
                cost_x100,
            ],
        )?;
        let product_id = tx.last_insert_rowid();

        for (unit, conversion) in sp.units {
            tx.execute(
                "INSERT INTO product_units (product_id, unit_id, conversion, sell_price, price_mode, is_default_sale)
                 VALUES (?1, ?2, ?3, ?4, 'AUTO', ?5)",
                params![
                    product_id,
                    unit_id(unit)?,
                    conversion,
                    auto_price(cost_x100, *conversion, margin_bp, price.rounding),
                    *unit == sp.default_sale,
                ],
            )?;
        }
    }

    tx.commit()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn seeds_master_data_once() {
        let mut conn = db::open_in_memory().unwrap();
        assert!(run(&mut conn).unwrap());
        assert!(!run(&mut conn).unwrap(), "seed kedua dilewati");

        let count = |table: &str| -> i64 {
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
        };
        assert_eq!(count("categories"), 5);
        assert_eq!(count("racks"), 10);
        assert_eq!(count("manufacturers"), 20);
        assert_eq!(count("products"), 50);

        let bad: i64 = conn
            .query_row(
                "SELECT count(*) FROM products p
                 WHERE (SELECT count(*) FROM product_units u WHERE u.product_id = p.id AND u.is_default_sale = 1) <> 1
                    OR (SELECT count(*) FROM product_units u
                        WHERE u.product_id = p.id AND u.unit_id = p.base_unit_id AND u.conversion = 1) <> 1
                    OR EXISTS (SELECT 1 FROM product_units u WHERE u.product_id = p.id AND u.sell_price = 0)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bad, 0, "tiap obat punya satu satuan dasar, satu satuan jual utama, dan harga");

        let unused: i64 = conn
            .query_row(
                "SELECT count(*) FROM manufacturers m WHERE NOT EXISTS
                     (SELECT 1 FROM products p WHERE p.manufacturer_id = m.id)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unused, 0, "setiap pabrik dipakai minimal satu obat");
    }
}
