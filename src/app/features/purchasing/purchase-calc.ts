import type { TaxMode } from '../../bindings/TaxMode';

// Cermin perhitungan Rust (src-tauri/src/purchasing/calc.rs) untuk pratinjau di form faktur.
// Angka final selalu dihitung ulang di Rust saat simpan dan posting.

const BP = 10_000n;

/** Bagi bulat, setengah ke atas (nilai tidak negatif). */
function divRound(num: bigint, den: bigint): number {
  if (den === 0n) return 0;
  return Number((num + den / 2n) / den);
}

export interface CalcLine {
  qty: number;
  bonusQty: number;
  conversion: number;
  unitPrice: number;
  discount1Bp: number;
  discount2Bp: number;
}

/** Nilai baris setelah diskon bertingkat, dibulatkan sekali di akhir. */
export function lineTotal(l: Pick<CalcLine, 'qty' | 'unitPrice' | 'discount1Bp' | 'discount2Bp'>): number {
  const gross = BigInt(l.qty) * BigInt(l.unitPrice);
  return divRound(gross * (BP - BigInt(l.discount1Bp)) * (BP - BigInt(l.discount2Bp)), BP * BP);
}

export interface CalcTotals {
  subtotal: number;
  linesTotal: number;
  discountTotal: number;
  taxTotal: number;
  grandTotal: number;
  /** HPP per satuan terkecil × 100, per baris. */
  unitCostsX100: number[];
}

export function computeInvoice(
  lines: CalcLine[],
  extraDiscount: number,
  taxMode: TaxMode,
  taxRateBp: number,
  taxInCost: boolean,
): CalcTotals {
  const subtotal = lines.reduce((s, l) => s + l.qty * l.unitPrice, 0);
  const totals = lines.map(lineTotal);
  const linesTotal = totals.reduce((s, t) => s + t, 0);
  const net = linesTotal - extraDiscount;
  const rate = BigInt(taxRateBp);

  let taxTotal = 0;
  let grandTotal = net;
  if (taxMode === 'EXCLUDED') {
    taxTotal = divRound(BigInt(net) * rate, BP);
    grandTotal = net + taxTotal;
  } else if (taxMode === 'INCLUDED') {
    taxTotal = net - divRound(BigInt(net) * BP, BP + rate);
  }

  let taxNum = 1n;
  let taxDen = 1n;
  if (taxMode === 'EXCLUDED' && taxInCost) {
    taxNum = BP + rate;
    taxDen = BP;
  } else if (taxMode === 'INCLUDED' && !taxInCost) {
    taxNum = BP;
    taxDen = BP + rate;
  }

  const unitCostsX100 = lines.map((l, i) => {
    const qtyBase = BigInt((l.qty + l.bonusQty) * l.conversion);
    if (qtyBase === 0n || linesTotal === 0 || net < 0) return 0;
    return divRound(BigInt(totals[i]) * BigInt(net) * taxNum * 100n, BigInt(linesTotal) * taxDen * qtyBase);
  });

  return { subtotal, linesTotal, discountTotal: subtotal - linesTotal + extraDiscount, taxTotal, grandTotal, unitCostsX100 };
}
