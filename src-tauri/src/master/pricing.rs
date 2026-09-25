//! Perhitungan harga jual otomatis.

/// Harga jual otomatis per satuan jual.
///
/// `cost_x100` = HPP per satuan terkecil (rupiah × 100), `conversion` = isi satuan jual,
/// `margin_bp` = margin dalam basis point. Hasil dibulatkan **ke atas** ke kelipatan `rounding`.
pub fn auto_price(cost_x100: i64, conversion: i64, margin_bp: i64, rounding: i64) -> i64 {
    let raw = i128::from(cost_x100) * i128::from(conversion) * i128::from(10_000 + margin_bp);
    let denom: i128 = 100 * 10_000;
    let price = (raw + denom - 1) / denom;
    ceil_to_multiple(i64::try_from(price).unwrap_or(i64::MAX), rounding)
}

pub fn ceil_to_multiple(value: i64, multiple: i64) -> i64 {
    if multiple <= 1 {
        return value;
    }
    (value + multiple - 1).div_euclid(multiple) * multiple
}

/// HPP per satuan jual dalam rupiah (dibulatkan ke atas), untuk dibandingkan dengan harga jual.
pub fn unit_cost(cost_x100: i64, conversion: i64) -> i64 {
    let v = i128::from(cost_x100) * i128::from(conversion);
    i64::try_from((v + 99) / 100).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_price_rounds_up() {
        // HPP Rp150,50/tablet, strip isi 10, margin 20% → 1806 → dibulatkan ke 1900.
        assert_eq!(auto_price(15_050, 10, 2_000, 100), 1_900);
        // Tanpa pembulatan.
        assert_eq!(auto_price(15_050, 10, 2_000, 1), 1_806);
        // Box isi 100, margin 0.
        assert_eq!(auto_price(15_050, 100, 0, 500), 15_500);
    }

    #[test]
    fn ceil_to_multiple_keeps_exact_values() {
        assert_eq!(ceil_to_multiple(1_000, 100), 1_000);
        assert_eq!(ceil_to_multiple(1_001, 100), 1_100);
        assert_eq!(ceil_to_multiple(0, 500), 0);
    }

    #[test]
    fn unit_cost_per_sale_unit() {
        assert_eq!(unit_cost(15_050, 10), 1_505);
        assert_eq!(unit_cost(15_051, 1), 151);
    }
}
