import { describe, expect, it } from 'vitest';

import { computeInvoice, lineTotal, type CalcLine } from './purchase-calc';

const line = (qty: number, conversion: number, unitPrice: number, d1 = 0, d2 = 0, bonusQty = 0): CalcLine => ({
  qty,
  bonusQty,
  conversion,
  unitPrice,
  discount1Bp: d1,
  discount2Bp: d2,
});

// Kasus yang sama dengan test Rust (src-tauri/src/purchasing/calc.rs) agar pratinjau selalu cocok.
describe('purchase-calc', () => {
  it('diskon bertingkat dibulatkan sekali', () => {
    expect(lineTotal(line(3, 1, 10_000, 1_000, 500))).toBe(25_650);
    expect(lineTotal(line(1, 1, 333, 1_000))).toBe(300);
  });

  it('PPN di luar harga masuk HPP untuk non-PKP', () => {
    const t = computeInvoice([line(2, 100, 100_000)], 0, 'EXCLUDED', 1_100, true);
    expect([t.subtotal, t.taxTotal, t.grandTotal]).toEqual([200_000, 22_000, 222_000]);
    expect(t.unitCostsX100).toEqual([111_000]);
    expect(computeInvoice([line(2, 100, 100_000)], 0, 'EXCLUDED', 1_100, false).unitCostsX100).toEqual([100_000]);
  });

  it('PPN termasuk harga dikeluarkan untuk PKP', () => {
    const t = computeInvoice([line(1, 10, 11_100)], 0, 'INCLUDED', 1_100, false);
    expect([t.taxTotal, t.grandTotal]).toEqual([1_100, 11_100]);
    expect(t.unitCostsX100).toEqual([100_000]);
  });

  it('diskon faktur dialokasikan proporsional', () => {
    const t = computeInvoice([line(3, 10, 10_000), line(1, 10, 10_000)], 4_000, 'NONE', 0, true);
    expect([t.discountTotal, t.grandTotal]).toEqual([4_000, 36_000]);
    expect(t.unitCostsX100).toEqual([90_000, 90_000]);
  });

  it('bonus menurunkan HPP', () => {
    const t = computeInvoice([line(10, 10, 10_000, 0, 0, 2)], 0, 'NONE', 0, true);
    expect(t.unitCostsX100).toEqual([83_333]);
  });
});
