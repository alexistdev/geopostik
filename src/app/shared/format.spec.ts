import { describe, expect, it } from 'vitest';

import { bpToPercent, formatCostX100, formatDateTime, formatRupiah, percentToBp } from './format';

describe('format', () => {
  it('formats rupiah with Indonesian grouping', () => {
    expect(formatRupiah(1500000)).toBe('Rp1.500.000');
    expect(formatRupiah(0)).toBe('Rp0');
    expect(formatRupiah(null)).toBe('–');
  });

  it('formats cost ×100 with decimals', () => {
    expect(formatCostX100(15050)).toBe('Rp150,5');
    expect(formatCostX100(15000)).toBe('Rp150');
  });

  it('converts between percent and basis points', () => {
    expect(percentToBp(12.5)).toBe(1250);
    expect(percentToBp(null)).toBeNull();
    expect(bpToPercent(2000)).toBe(20);
  });

  it('formats SQLite local datetime', () => {
    expect(formatDateTime('2026-09-26 14:05:09')).toBe('26/09/2026 14:05');
    expect(formatDateTime(null)).toBe('–');
  });
});
