import type { AuditRow } from '../../bindings/AuditRow';
import type { DrugClass } from '../../bindings/DrugClass';
import { bpToPercent, formatCostX100, formatRupiah } from '../../shared/format';
import { drugClassInfo, PRICE_MODES, ROLES } from '../../shared/labels';

type TagSeverity = 'success' | 'info' | 'warn' | 'danger' | 'secondary' | 'contrast';

/** Jenis data yang tercatat di log (kolom `entity`). */
export const ENTITIES: { value: string; label: string }[] = [
  { value: 'products', label: 'Obat' },
  { value: 'categories', label: 'Kategori' },
  { value: 'racks', label: 'Rak' },
  { value: 'manufacturers', label: 'Pabrik' },
  { value: 'units', label: 'Satuan' },
  { value: 'stock_opnames', label: 'Stok opname' },
  { value: 'batches', label: 'Batch' },
  { value: 'users', label: 'Pengguna' },
];

/** Aksi yang bisa difilter. Aksi lama berawalan entitas (PRODUCT_UPDATE) ikut cocok. */
export const ACTIONS: { value: string; label: string; severity: TagSeverity }[] = [
  { value: 'CREATE', label: 'Tambah', severity: 'success' },
  { value: 'UPDATE', label: 'Ubah', severity: 'info' },
  { value: 'DELETE', label: 'Hapus', severity: 'danger' },
  { value: 'ACTIVATE', label: 'Aktifkan', severity: 'success' },
  { value: 'DEACTIVATE', label: 'Nonaktifkan', severity: 'warn' },
  { value: 'PRICE_CHANGE', label: 'Ubah harga', severity: 'info' },
  { value: 'PRICE_RECALC', label: 'Hitung ulang harga', severity: 'secondary' },
  { value: 'SUBMIT', label: 'Ajukan', severity: 'info' },
  { value: 'REOPEN', label: 'Kembali ke draft', severity: 'secondary' },
  { value: 'APPROVE', label: 'Setujui', severity: 'success' },
  { value: 'CANCEL', label: 'Batalkan', severity: 'danger' },
  { value: 'LOCK', label: 'Kunci', severity: 'warn' },
  { value: 'UNLOCK', label: 'Buka kunci', severity: 'success' },
  { value: 'OPENING_LOCK', label: 'Kunci stok awal', severity: 'contrast' },
  { value: 'PASSWORD_CHANGE', label: 'Ganti password', severity: 'warn' },
  { value: 'PIN_CHANGE', label: 'Ganti PIN', severity: 'warn' },
  { value: 'LOGIN', label: 'Login', severity: 'secondary' },
  { value: 'LOGOUT', label: 'Logout', severity: 'secondary' },
  { value: 'INITIAL_SETUP', label: 'Setup awal', severity: 'contrast' },
];

/** Aksi lama sebelum menu Log dibuat. */
const LEGACY_ACTIONS: Record<string, string> = {
  CATEGORY_MARGIN_CHANGE: 'UPDATE',
  MASTER_ACTIVATE: 'ACTIVATE',
  MASTER_DEACTIVATE: 'DEACTIVATE',
  MASTER_DELETE: 'DELETE',
  PRODUCT_CREATE: 'CREATE',
  PRODUCT_UPDATE: 'UPDATE',
  PRODUCT_ACTIVATE: 'ACTIVATE',
  PRODUCT_DEACTIVATE: 'DEACTIVATE',
};

export function actionInfo(action: string): { label: string; severity: TagSeverity } {
  const known = ACTIONS.find((a) => a.value === (LEGACY_ACTIONS[action] ?? action));
  return known ?? { label: action, severity: 'secondary' };
}

export function entityLabel(entity: string | null): string {
  if (!entity) return '–';
  return ENTITIES.find((e) => e.value === entity)?.label ?? entity;
}

const FIELD_LABELS: Record<string, string> = {
  code: 'Kode',
  name: 'Nama',
  genericName: 'Nama generik',
  manufacturer: 'Pabrik',
  category: 'Kategori',
  rack: 'Rak',
  drugClass: 'Golongan',
  isOwa: 'OWA',
  baseUnit: 'Satuan dasar',
  minStockBase: 'Stok minimal (satuan dasar)',
  isActive: 'Aktif',
  marginBp: 'Margin',
  lastCostX100: 'HPP per satuan dasar',
  units: 'Satuan',
  conversion: 'Isi (satuan dasar)',
  isDefaultSale: 'Satuan jual utama',
  priceMode: 'Mode harga',
  sellPrice: 'Harga jual',
  barcodes: 'Barcode',
  tiers: 'Tier',
  price: 'Harga',
  username: 'Username',
  alsoPharmacist: 'Juga apoteker',
  opnameType: 'Jenis opname',
  batchNumber: 'No. batch',
  expiryDate: 'Tanggal ED',
  roles: 'Peran',
  licenseType: 'Jenis izin',
  licenseNumber: 'Nomor SIPA/SIPTTK',
};

export function fieldLabel(path: string[]): string {
  const parts: string[] = [];
  for (let i = 0; i < path.length; i++) {
    const key = path[i];
    // `units` / `tiers` diikuti nama satuan / tier: digabung jadi "Satuan Strip", "Tier ≥ 10".
    if ((key === 'units' || key === 'tiers') && i + 1 < path.length) {
      parts.push(`${FIELD_LABELS[key]} ${path[++i]}`);
    } else {
      parts.push(FIELD_LABELS[key] ?? key);
    }
  }
  return parts.join(' › ');
}

export function formatValue(key: string, value: unknown): string {
  if (value === null || value === undefined || value === '') return '–';
  if (typeof value === 'boolean') return value ? 'Ya' : 'Tidak';
  if (typeof value === 'number') {
    switch (key) {
      case 'marginBp':
        return `${bpToPercent(value)}%`;
      case 'lastCostX100':
        return formatCostX100(value);
      case 'sellPrice':
      case 'price':
        return formatRupiah(value);
    }
    return String(value);
  }
  if (typeof value === 'string') {
    if (key === 'drugClass') return drugClassInfo(value as DrugClass)?.label ?? value;
    if (key === 'opnameType') return value === 'OPENING' ? 'Stok awal' : value === 'PERIODIC' ? 'Opname berkala' : value;
    if (key === 'priceMode') return PRICE_MODES.find((m) => m.value === value)?.label ?? value;
    if (key === 'roles') {
      return value
        .split(', ')
        .map((r) => ROLES.find((x) => x.value === r)?.label ?? r)
        .join(', ');
    }
    return value;
  }
  return JSON.stringify(value);
}

export interface FieldChange {
  label: string;
  before: string;
  after: string;
  changed: boolean;
}

type Json = Record<string, unknown>;

function isObject(v: unknown): v is Json {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Semua nilai daun sebagai `[path, value]`. Objek kosong (misal tanpa tier) diabaikan. */
function flatten(value: unknown, path: string[] = [], out = new Map<string, [string[], unknown]>()) {
  if (isObject(value)) {
    for (const [k, v] of Object.entries(value)) flatten(v, [...path, k], out);
  } else if (path.length) {
    out.set(path.join('\u0000'), [path, value]);
  }
  return out;
}

/**
 * Bandingkan snapshot sebelum/sesudah per field. Data baru: `before` null; data terhapus:
 * `after` null. Field yang sama tetap dikembalikan (changed = false) agar isi lengkap bisa dilihat.
 */
export function diffSnapshots(before: unknown, after: unknown): FieldChange[] {
  const b = flatten(before);
  const a = flatten(after);
  const keys = [...new Set([...b.keys(), ...a.keys()])];
  return keys.map((k) => {
    const path = (b.get(k) ?? a.get(k))![0];
    const leaf = path[path.length - 1];
    const bv = b.get(k)?.[1];
    const av = a.get(k)?.[1];
    return {
      label: fieldLabel(path),
      before: before == null ? '' : formatValue(leaf, bv),
      after: after == null ? '' : formatValue(leaf, av),
      changed: before != null && after != null && JSON.stringify(bv ?? null) !== JSON.stringify(av ?? null),
    };
  });
}

export interface ParsedDetail {
  code: string | null;
  name: string | null;
  /** Ada snapshot sebelum/sesudah (format baru). */
  hasSnapshot: boolean;
  before: unknown;
  after: unknown;
  /** Isi detail lain (format lama atau aksi tanpa snapshot). */
  extra: Json;
  raw: string | null;
}

export function parseDetail(row: Pick<AuditRow, 'detail'>): ParsedDetail {
  let json: Json = {};
  try {
    const parsed = row.detail ? JSON.parse(row.detail) : {};
    if (isObject(parsed)) json = parsed;
  } catch {
    // Detail bukan JSON: tampilkan mentah saja.
  }
  const { code, name, before, after, ...extra } = json;
  return {
    code: typeof code === 'string' ? code : null,
    name: typeof name === 'string' ? name : null,
    hasSnapshot: 'before' in json || 'after' in json,
    before: before ?? null,
    after: after ?? null,
    extra,
    raw: row.detail,
  };
}

/** Ringkasan satu baris untuk tabel, misal "Nama, Margin" atau "3 field berubah". */
export function summarize(row: AuditRow): string {
  const d = parseDetail(row);
  if (row.reason) return row.reason;
  if (!d.hasSnapshot) {
    const entries = Object.entries(d.extra);
    return entries.map(([k, v]) => `${fieldLabel([k])}: ${formatValue(k, v)}`).join(', ');
  }
  if (d.before == null) return 'Data baru';
  if (d.after == null) return 'Data dihapus';
  const changed = diffSnapshots(d.before, d.after).filter((c) => c.changed);
  if (changed.length <= 3) return changed.map((c) => c.label).join(', ');
  return `${changed.length} field berubah`;
}
