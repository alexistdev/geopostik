//! Harga jual per baris: harga eceran satuan, atau harga tier grosir bila jumlahnya mencapai
//! batas (docs/SCHEMA.md `price_tiers`). Dipakai resep, dan nantinya kasir.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinePrice {
    pub unit_price: i64,
    /// Tier yang dipakai; `None` = harga eceran.
    pub tier_id: Option<i64>,
    pub tier_min_qty: Option<i64>,
}

/// Harga per satuan untuk `qty` satuan jual: tier aktif dengan `min_qty` terbesar yang ≤ qty,
/// atau `product_units.sell_price` bila tidak ada.
pub fn price_for_qty(conn: &Connection, product_unit_id: i64, qty: i64) -> AppResult<LinePrice> {
    let tier: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, min_qty, price FROM price_tiers
             WHERE product_unit_id = ?1 AND is_active = 1 AND min_qty <= ?2
             ORDER BY min_qty DESC LIMIT 1",
            params![product_unit_id, qty],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((id, min_qty, price)) = tier {
        return Ok(LinePrice { unit_price: price, tier_id: Some(id), tier_min_qty: Some(min_qty) });
    }
    let price: i64 = conn
        .query_row("SELECT sell_price FROM product_units WHERE id = ?1", [product_unit_id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::NotFound("Satuan obat tidak ditemukan".into()))?;
    Ok(LinePrice { unit_price: price, tier_id: None, tier_min_qty: None })
}
