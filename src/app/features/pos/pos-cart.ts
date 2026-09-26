import type { PosProduct } from '../../bindings/PosProduct';
import type { PosUnit } from '../../bindings/PosUnit';

/**
 * Hitungan keranjang di layar kasir. Aturannya sama dengan Rust (`sales/service.rs`); Rust tetap
 * menghitung ulang dan menolak bila totalnya berbeda, jadi ini hanya untuk tampilan.
 */

export interface CartLine {
  /** Kunci baris di layar. */
  key: number;
  product: PosProduct;
  productUnitId: number;
  qty: number;
  /** Diskon rupiah untuk seluruh baris. */
  discount: number;
}

export function unitOf(line: CartLine): PosUnit {
  return line.product.units.find((u) => u.productUnitId === line.productUnitId) ?? line.product.units[0];
}

/** Harga per satuan: tier dengan `minQty` terbesar yang ≤ qty, atau harga eceran. */
export function priceFor(unit: PosUnit, qty: number): { price: number; tierMinQty: number | null } {
  let best: { price: number; tierMinQty: number | null } = { price: unit.sellPrice, tierMinQty: null };
  for (const t of unit.tiers) {
    if (t.minQty <= qty && (best.tierMinQty == null || t.minQty > best.tierMinQty)) {
      best = { price: t.price, tierMinQty: t.minQty };
    }
  }
  return best;
}

export function lineGross(line: CartLine): number {
  return priceFor(unitOf(line), line.qty).price * line.qty;
}

export function lineTotal(line: CartLine): number {
  return lineGross(line) - line.discount;
}

/** Jumlah satuan terkecil yang diminta baris ini. */
export function lineBase(line: CartLine): number {
  return unitOf(line).conversion * line.qty;
}

/** Stok bisa-jual yang belum dipakai baris lain untuk obat yang sama (satuan terkecil). */
export function remainingBase(lines: CartLine[], line: CartLine): number {
  const others = lines
    .filter((l) => l.key !== line.key && l.product.productId === line.product.productId)
    .reduce((s, l) => s + lineBase(l), 0);
  return Math.max(0, line.product.sellableBase - others);
}

/**
 * Qty maksimal satu baris dalam satuan `productUnitId` (default satuan baris itu), agar total
 * semua baris obat yang sama tidak melebihi stok bisa-jual.
 */
export function maxQty(lines: CartLine[], line: CartLine, productUnitId = line.productUnitId): number {
  const unit = line.product.units.find((u) => u.productUnitId === productUnitId) ?? unitOf(line);
  return Math.floor(remainingBase(lines, line) / unit.conversion);
}

/** Diskon melebihi batas tanpa otorisasi (basis point dari harga baris). */
export function discountOverLimit(line: CartLine, maxDiscountBp: number): boolean {
  return line.discount * 10_000 > lineGross(line) * maxDiscountBp;
}

/** Pembulatan ke bawah ke kelipatan `unit` (nilai ≤ 0). */
export function roundingFor(total: number, unit: number): number {
  return unit > 1 ? -(total % unit) : 0;
}

export interface CartTotals {
  subtotal: number;
  discount: number;
  rounding: number;
  total: number;
}

export function cartTotals(lines: CartLine[], roundingUnit: number): CartTotals {
  const subtotal = lines.reduce((s, l) => s + lineGross(l), 0);
  const discount = lines.reduce((s, l) => s + l.discount, 0);
  const rounding = roundingFor(subtotal - discount, roundingUnit);
  return { subtotal, discount, rounding, total: subtotal - discount + rounding };
}

/** Total kebutuhan satuan terkecil per obat (satu obat bisa ada di beberapa baris/satuan). */
export function baseNeedByProduct(lines: CartLine[]): Map<number, number> {
  const need = new Map<number, number>();
  for (const l of lines) {
    need.set(l.product.productId, (need.get(l.product.productId) ?? 0) + lineBase(l));
  }
  return need;
}

export interface PaymentPlan {
  /** Bagian non-tunai (QRIS/Debit), maksimal sebesar total. */
  nonCash: number;
  /** Bagian yang dibayar tunai. */
  cashPart: number;
  /** Uang tunai yang diterima; kosong = pas. */
  tendered: number;
  change: number;
  /** Uang yang diterima belum cukup. */
  short: number;
}

export function paymentPlan(total: number, nonCash: number | null, tendered: number | null): PaymentPlan {
  const nc = Math.min(Math.max(nonCash ?? 0, 0), total);
  const cashPart = total - nc;
  const got = tendered ?? cashPart;
  return {
    nonCash: nc,
    cashPart,
    tendered: got,
    change: Math.max(got - cashPart, 0),
    short: Math.max(cashPart - got, 0),
  };
}

/** Kunci idempotensi satu checkout. */
export function newClientRef(): string {
  return crypto.randomUUID();
}
