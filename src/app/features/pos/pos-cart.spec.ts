import { describe, expect, it } from 'vitest';

import type { PosProduct } from '../../bindings/PosProduct';
import {
  type CartLine,
  baseNeedByProduct,
  cartTotals,
  discountOverLimit,
  maxQty,
  paymentPlan,
  priceFor,
  roundingFor,
} from './pos-cart';

const paracetamol: PosProduct = {
  productId: 1,
  code: 'P1',
  name: 'Paracetamol',
  genericName: null,
  drugClass: 'FREE',
  isOwa: false,
  baseUnitName: 'Tablet',
  sellableBase: 55,
  matchedUnitId: null,
  units: [
    { productUnitId: 11, unitName: 'Tablet', conversion: 1, sellPrice: 500, isDefault: true, tiers: [] },
    {
      productUnitId: 12,
      unitName: 'Strip',
      conversion: 10,
      sellPrice: 4500,
      isDefault: false,
      tiers: [
        { minQty: 5, price: 4000 },
        { minQty: 20, price: 3800 },
      ],
    },
  ],
};

const line = (productUnitId: number, qty: number, discount = 0): CartLine => ({
  key: qty,
  product: paracetamol,
  productUnitId,
  qty,
  discount,
});

describe('pos-cart', () => {
  it('uses the largest tier not above the quantity', () => {
    const strip = paracetamol.units[1];
    expect(priceFor(strip, 4)).toEqual({ price: 4500, tierMinQty: null });
    expect(priceFor(strip, 5)).toEqual({ price: 4000, tierMinQty: 5 });
    expect(priceFor(strip, 25)).toEqual({ price: 3800, tierMinQty: 20 });
  });

  it('rounds totals down like the backend', () => {
    expect(roundingFor(1450, 100)).toBe(-50);
    expect(roundingFor(1400, 100)).toBe(-0);
    expect(roundingFor(1450, 0)).toBe(0);
    const t = cartTotals([line(11, 3, 50)], 100);
    expect(t).toEqual({ subtotal: 1500, discount: 50, rounding: -50, total: 1400 });
  });

  it('flags discounts above the limit', () => {
    expect(discountOverLimit(line(11, 10, 500), 1000)).toBe(false);
    expect(discountOverLimit(line(11, 10, 501), 1000)).toBe(true);
  });

  it('sums base quantity per product across units', () => {
    expect(baseNeedByProduct([line(12, 2), line(11, 7)]).get(1)).toBe(27);
  });

  it('limits quantity to the stock left after other lines', () => {
    // 55 tablet bisa dijual: 2 strip (20) di baris lain → tablet maks 35, strip maks 3.
    const strip = { ...line(12, 2), key: 1 };
    const tablet = { ...line(11, 1), key: 2 };
    expect(maxQty([strip, tablet], tablet)).toBe(35);
    expect(maxQty([strip, tablet], strip)).toBe(5);
    expect(maxQty([strip, tablet], tablet, 12)).toBe(3);
    const lowStock = { ...tablet, product: { ...paracetamol, sellableBase: 8 } };
    expect(maxQty([lowStock], lowStock, 12)).toBe(0);
  });

  it('splits payment into non-cash and cash with change', () => {
    expect(paymentPlan(10_000, 7_000, 5_000)).toEqual({ nonCash: 7_000, cashPart: 3_000, tendered: 5_000, change: 2_000, short: 0 });
    expect(paymentPlan(10_000, null, null)).toEqual({ nonCash: 0, cashPart: 10_000, tendered: 10_000, change: 0, short: 0 });
    expect(paymentPlan(10_000, 20_000, null).nonCash).toBe(10_000);
    expect(paymentPlan(10_000, 0, 8_000).short).toBe(2_000);
  });
});
