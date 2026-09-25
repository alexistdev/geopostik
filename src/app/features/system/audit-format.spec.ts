import { describe, expect, it } from 'vitest';

import type { AuditRow } from '../../bindings/AuditRow';
import { actionInfo, diffSnapshots, fieldLabel, formatValue, parseDetail, summarize } from './audit-format';

function row(detail: unknown, reason: string | null = null): AuditRow {
  return {
    id: 1,
    createdAt: '2026-09-26 10:00:00',
    userId: 1,
    username: 'budi',
    fullName: 'Budi',
    authorizedBy: null,
    action: 'UPDATE',
    entity: 'products',
    entityId: 1,
    detail: detail == null ? null : JSON.stringify(detail),
    reason,
  };
}

describe('audit-format', () => {
  it('labels nested product fields', () => {
    expect(fieldLabel(['units', 'Strip', 'sellPrice'])).toBe('Satuan Strip › Harga jual');
    expect(fieldLabel(['units', 'Box', 'tiers', '≥ 10', 'price'])).toBe('Satuan Box › Tier ≥ 10 › Harga');
  });

  it('formats values by field', () => {
    expect(formatValue('marginBp', 2500)).toBe('25%');
    expect(formatValue('sellPrice', 1500)).toBe('Rp1.500');
    expect(formatValue('lastCostX100', 15050)).toBe('Rp150,5');
    expect(formatValue('isActive', false)).toBe('Tidak');
    expect(formatValue('drugClass', 'HARD')).toBe('Keras');
    expect(formatValue('rack', null)).toBe('–');
    expect(formatValue('roles', 'OWNER, PHARMACIST')).toBe('Pemilik, Apoteker');
  });

  it('diffs snapshots, including added and removed units', () => {
    const before = { name: 'A', units: { Strip: { sellPrice: 1000 } } };
    const after = { name: 'A', units: { Strip: { sellPrice: 1200 }, Box: { sellPrice: 10000 } } };
    const diff = diffSnapshots(before, after);
    expect(diff.filter((d) => d.changed).map((d) => [d.label, d.before, d.after])).toEqual([
      ['Satuan Strip › Harga jual', 'Rp1.000', 'Rp1.200'],
      ['Satuan Box › Harga jual', '–', 'Rp10.000'],
    ]);
    expect(diff.find((d) => d.label === 'Nama')?.changed).toBe(false);
  });

  it('shows created data without "before" values', () => {
    const diff = diffSnapshots(null, { name: 'Vitamin' });
    expect(diff).toEqual([{ label: 'Nama', before: '', after: 'Vitamin', changed: false }]);
  });

  it('summarizes rows', () => {
    expect(summarize(row({ code: 'K1', before: { name: 'A', isActive: true }, after: { name: 'B', isActive: true } }))).toBe(
      'Nama',
    );
    expect(summarize(row({ code: 'K1', before: null, after: { name: 'B' } }))).toBe('Data baru');
    expect(summarize(row({ code: 'K1', before: { name: 'B' }, after: null }))).toBe('Data dihapus');
    expect(summarize(row({ marginBp: 1000 }))).toBe('Margin: 10%');
    expect(summarize(row(null, 'Margin kategori X diubah'))).toBe('Margin kategori X diubah');
  });

  it('reads legacy entries', () => {
    expect(actionInfo('PRODUCT_CREATE').label).toBe('Tambah');
    expect(actionInfo('MASTER_DELETE').label).toBe('Hapus');
    const d = parseDetail(row({ code: 'OBT00001', name: 'Paracetamol' }));
    expect([d.code, d.name, d.hasSnapshot]).toEqual(['OBT00001', 'Paracetamol', false]);
  });
});
