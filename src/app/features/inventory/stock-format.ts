import type { MovementType } from '../../bindings/MovementType';
import type { OpnameStatus } from '../../bindings/OpnameStatus';
import type { OpnameType } from '../../bindings/OpnameType';
import type { StockFilter } from '../../bindings/StockFilter';

type TagSeverity = 'success' | 'info' | 'warn' | 'danger' | 'secondary' | 'contrast';

/** Batch dengan ED sampai sekian hari ke depan dianggap "hampir ED" (sama dengan Rust). */
export const NEAR_EXPIRY_DAYS = 90;

export const STOCK_FILTERS: { value: StockFilter; label: string; icon: string }[] = [
  { value: 'ALL', label: 'Semua', icon: 'pi pi-list' },
  { value: 'LOW', label: 'Di bawah minimal', icon: 'pi pi-arrow-down' },
  { value: 'EMPTY', label: 'Stok habis', icon: 'pi pi-inbox' },
  { value: 'NEAR_EXPIRY', label: 'Hampir ED', icon: 'pi pi-clock' },
  { value: 'EXPIRED', label: 'Sudah ED', icon: 'pi pi-exclamation-triangle' },
];

export const MOVEMENT_TYPES: Record<MovementType, { label: string; severity: TagSeverity }> = {
  OPENING: { label: 'Stok awal', severity: 'contrast' },
  PURCHASE: { label: 'Pembelian', severity: 'success' },
  PURCHASE_VOID: { label: 'Batal beli', severity: 'danger' },
  SALE: { label: 'Penjualan', severity: 'info' },
  SALE_VOID: { label: 'Batal jual', severity: 'warn' },
  SALE_RETURN: { label: 'Retur jual', severity: 'warn' },
  SUPPLIER_RETURN: { label: 'Retur supplier', severity: 'warn' },
  ADJUSTMENT: { label: 'Penyesuaian', severity: 'secondary' },
  DESTRUCTION: { label: 'Pemusnahan', severity: 'danger' },
};

export const OPNAME_TYPES: Record<OpnameType, { label: string; icon: string; hint: string }> = {
  OPENING: {
    label: 'Stok awal',
    icon: 'pi pi-flag',
    hint: 'Input stok pertama kali per batch (no batch, ED, HPP, jumlah). Pengganti migrasi data.',
  },
  PERIODIC: {
    label: 'Opname berkala',
    icon: 'pi pi-sync',
    hint: 'Hitung ulang batch yang ada per rak/kategori; selisih menjadi penyesuaian stok.',
  },
};

export const OPNAME_STATUSES: { value: OpnameStatus; label: string; severity: TagSeverity }[] = [
  { value: 'DRAFT', label: 'Draft', severity: 'secondary' },
  { value: 'SUBMITTED', label: 'Diajukan', severity: 'warn' },
  { value: 'APPROVED', label: 'Disetujui', severity: 'success' },
  { value: 'CANCELLED', label: 'Dibatalkan', severity: 'danger' },
];

export function opnameStatusInfo(status: OpnameStatus) {
  return OPNAME_STATUSES.find((s) => s.value === status)!;
}

/** Tanggal lokal `YYYY-MM-DD`. */
export function isoDate(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** "2027-01-31" → "31/01/2027". */
export function formatDate(value: string | null | undefined): string {
  const m = value?.match(/^(\d{4})-(\d{2})-(\d{2})/);
  return m ? `${m[3]}/${m[2]}/${m[1]}` : '–';
}

export type ExpiryState = 'expired' | 'near' | 'ok';

/** Status ED terhadap `today` (`YYYY-MM-DD`); perbandingan string aman untuk format ISO. */
export function expiryState(expiry: string | null | undefined, today: string): ExpiryState {
  if (!expiry) return 'ok';
  if (expiry <= today) return 'expired';
  const near = new Date(`${today}T00:00:00`);
  near.setDate(near.getDate() + NEAR_EXPIRY_DAYS);
  return expiry <= isoDate(near) ? 'near' : 'ok';
}

/** Rupiah (boleh desimal) → rupiah × 100. */
export function toX100(value: number | null | undefined): number | null {
  return value == null ? null : Math.round(value * 100);
}
