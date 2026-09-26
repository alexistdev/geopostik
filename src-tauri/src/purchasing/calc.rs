//! Perhitungan faktur pembelian: diskon bertingkat, diskon faktur, PPN, dan HPP per batch.
//! Semua uang integer rupiah; HPP per satuan terkecil dalam rupiah × 100 (SCHEMA.md §0).

use super::model::TaxMode;

const BP: i128 = 10_000;

/// Bagi bulat, setengah dibulatkan ke atas (semua nilai di sini tidak negatif).
fn div_round(num: i128, den: i128) -> i64 {
    if den == 0 {
        return 0;
    }
    i64::try_from((num + den / 2) / den).unwrap_or(i64::MAX)
}

/// Nilai baris setelah diskon bertingkat: qty × harga × (1 − d1) × (1 − d2), dibulatkan sekali
/// di akhir agar tidak ada selisih pembulatan ganda.
pub fn line_total(qty: i64, unit_price: i64, discount1_bp: i64, discount2_bp: i64) -> i64 {
    let gross = i128::from(qty) * i128::from(unit_price);
    div_round(gross * (BP - i128::from(discount1_bp)) * (BP - i128::from(discount2_bp)), BP * BP)
}

/// Satu baris untuk hitung faktur.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub qty: i64,
    pub bonus_qty: i64,
    pub conversion: i64,
    pub unit_price: i64,
    pub discount1_bp: i64,
    pub discount2_bp: i64,
}

impl Line {
    pub fn gross(&self) -> i64 {
        self.qty * self.unit_price
    }

    pub fn total(&self) -> i64 {
        line_total(self.qty, self.unit_price, self.discount1_bp, self.discount2_bp)
    }

    /// Stok yang masuk (qty + bonus) dalam satuan terkecil.
    pub fn qty_base(&self) -> i64 {
        (self.qty + self.bonus_qty) * self.conversion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totals {
    /// Σ qty × harga.
    pub subtotal: i64,
    /// Σ nilai baris setelah diskon baris.
    pub lines_total: i64,
    /// Diskon baris + diskon faktur.
    pub discount_total: i64,
    pub tax_total: i64,
    pub grand_total: i64,
    /// HPP per satuan terkecil (× 100) per baris, urutan sama dengan input.
    pub unit_costs_x100: Vec<i64>,
}

/// Hitung total faktur dan HPP per baris.
///
/// - Diskon faktur (`extra_discount`) dialokasikan proporsional ke nilai baris.
/// - `EXCLUDED`: PPN = (nilai − diskon faktur) × tarif; `INCLUDED`: PPN sudah di dalam harga,
///   DPP = nilai × 100 / (100 + tarif); `NONE`: tanpa PPN.
/// - `tax_in_cost` (apotek non-PKP): PPN tidak bisa dikreditkan sehingga ikut ke HPP. Apotek PKP:
///   HPP tanpa PPN.
/// - Bonus menambah stok tanpa menambah nilai, sehingga HPP baris dibagi ke qty + bonus.
pub fn compute(lines: &[Line], extra_discount: i64, tax_mode: TaxMode, tax_rate_bp: i64, tax_in_cost: bool) -> Totals {
    let subtotal: i64 = lines.iter().map(Line::gross).sum();
    let totals: Vec<i64> = lines.iter().map(Line::total).collect();
    let lines_total: i64 = totals.iter().sum();
    let net = lines_total - extra_discount;
    let rate = i128::from(tax_rate_bp);

    let (tax_total, grand_total) = match tax_mode {
        TaxMode::Excluded => {
            let tax = div_round(i128::from(net) * rate, BP);
            (tax, net + tax)
        }
        TaxMode::Included => (net - div_round(i128::from(net) * BP, BP + rate), net),
        TaxMode::None => (0, net),
    };

    // Faktor pajak terhadap nilai baris (setelah diskon faktur) untuk HPP: num / den.
    let (tax_num, tax_den) = match (tax_mode, tax_in_cost) {
        (TaxMode::Excluded, true) => (BP + rate, BP),
        (TaxMode::Included, false) => (BP, BP + rate),
        _ => (1, 1),
    };

    let unit_costs_x100 = lines
        .iter()
        .zip(&totals)
        .map(|(l, &total)| {
            let qty_base = i128::from(l.qty_base());
            if qty_base == 0 || lines_total == 0 {
                return 0;
            }
            // total × (net / lines_total) × faktor pajak × 100 / qty_base
            let num = i128::from(total) * i128::from(net) * tax_num * 100;
            let den = i128::from(lines_total) * tax_den * qty_base;
            div_round(num, den)
        })
        .collect();

    Totals {
        subtotal,
        lines_total,
        discount_total: subtotal - lines_total + extra_discount,
        tax_total,
        grand_total,
        unit_costs_x100,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(qty: i64, conversion: i64, price: i64, d1: i64, d2: i64) -> Line {
        Line { qty, bonus_qty: 0, conversion, unit_price: price, discount1_bp: d1, discount2_bp: d2 }
    }

    #[test]
    fn cascading_discount_rounds_once() {
        // 3 × 10.000 = 30.000, −10% = 27.000, −5% = 25.650.
        assert_eq!(line_total(3, 10_000, 1_000, 500), 25_650);
        assert_eq!(line_total(1, 333, 1_000, 0), 300); // 299,7 → 300
        assert_eq!(line_total(2, 5_000, 0, 0), 10_000);
    }

    #[test]
    fn excluded_tax_goes_into_cost_for_non_pkp() {
        // 2 box isi 100 @ Rp100.000, PPN 11% di luar harga.
        let t = compute(&[line(2, 100, 100_000, 0, 0)], 0, TaxMode::Excluded, 1_100, true);
        assert_eq!((t.subtotal, t.tax_total, t.grand_total), (200_000, 22_000, 222_000));
        // HPP per tablet = 222.000 / 200 = Rp1.110.
        assert_eq!(t.unit_costs_x100, vec![111_000]);

        // PKP: PPN dikreditkan, HPP tanpa PPN.
        let pkp = compute(&[line(2, 100, 100_000, 0, 0)], 0, TaxMode::Excluded, 1_100, false);
        assert_eq!(pkp.grand_total, 222_000);
        assert_eq!(pkp.unit_costs_x100, vec![100_000]);
    }

    #[test]
    fn included_tax_is_extracted_for_pkp() {
        let t = compute(&[line(1, 10, 11_100, 0, 0)], 0, TaxMode::Included, 1_100, false);
        assert_eq!((t.tax_total, t.grand_total), (1_100, 11_100));
        // DPP 10.000 / 10 = Rp1.000.
        assert_eq!(t.unit_costs_x100, vec![100_000]);

        let non_pkp = compute(&[line(1, 10, 11_100, 0, 0)], 0, TaxMode::Included, 1_100, true);
        assert_eq!(non_pkp.unit_costs_x100, vec![111_000]);
    }

    #[test]
    fn header_discount_is_allocated_proportionally() {
        // Baris 30.000 dan 10.000, diskon faktur 4.000 (10%), tanpa PPN.
        let t = compute(&[line(3, 10, 10_000, 0, 0), line(1, 10, 10_000, 0, 0)], 4_000, TaxMode::None, 0, true);
        assert_eq!((t.discount_total, t.grand_total), (4_000, 36_000));
        // 27.000 / 30 tablet = 900; 9.000 / 10 tablet = 900.
        assert_eq!(t.unit_costs_x100, vec![90_000, 90_000]);
    }

    #[test]
    fn bonus_lowers_unit_cost() {
        // Beli 10 strip isi 10 @ Rp10.000 + bonus 2 strip → 120 tablet senilai 100.000.
        let l = Line { bonus_qty: 2, ..line(10, 10, 10_000, 0, 0) };
        let t = compute(&[l], 0, TaxMode::None, 0, true);
        assert_eq!(l.qty_base(), 120);
        assert_eq!(t.grand_total, 100_000);
        assert_eq!(t.unit_costs_x100, vec![83_333]); // Rp833,33
    }

    #[test]
    fn fractional_cost_keeps_two_decimals() {
        // Rp15.050 / 100 tablet = Rp150,50.
        let t = compute(&[line(1, 100, 15_050, 0, 0)], 0, TaxMode::None, 0, true);
        assert_eq!(t.unit_costs_x100, vec![15_050]);
    }

    #[test]
    fn zero_value_invoice_has_zero_cost() {
        let t = compute(&[line(1, 10, 0, 0, 0)], 0, TaxMode::Excluded, 1_100, true);
        assert_eq!((t.grand_total, t.unit_costs_x100.clone()), (0, vec![0]));
    }
}
