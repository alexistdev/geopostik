import type { DebtFilter } from '../../bindings/DebtFilter';
import type { PurchasePaymentType } from '../../bindings/PurchasePaymentType';
import type { PurchaseStatus } from '../../bindings/PurchaseStatus';
import type { SupplierPaymentMethod } from '../../bindings/SupplierPaymentMethod';
import type { TaxMode } from '../../bindings/TaxMode';

type TagSeverity = 'success' | 'info' | 'warn' | 'danger' | 'secondary' | 'contrast';

export const PURCHASE_STATUSES: { value: PurchaseStatus; label: string; severity: TagSeverity }[] = [
  { value: 'DRAFT', label: 'Draft', severity: 'secondary' },
  { value: 'POSTED', label: 'Diposting', severity: 'success' },
  { value: 'VOID', label: 'Batal', severity: 'danger' },
];

export function purchaseStatusInfo(status: PurchaseStatus) {
  return PURCHASE_STATUSES.find((s) => s.value === status)!;
}

export const PAYMENT_TYPES: { value: PurchasePaymentType; label: string }[] = [
  { value: 'CASH', label: 'Tunai' },
  { value: 'CREDIT', label: 'Kredit' },
];

export const TAX_MODES: { value: TaxMode; label: string; hint: string }[] = [
  { value: 'EXCLUDED', label: 'Belum termasuk PPN', hint: 'PPN ditambahkan di atas nilai barang' },
  { value: 'INCLUDED', label: 'Termasuk PPN', hint: 'Harga di faktur sudah termasuk PPN' },
  { value: 'NONE', label: 'Tanpa PPN', hint: 'Faktur tidak dikenai PPN' },
];

export const PAYMENT_METHODS: { value: SupplierPaymentMethod; label: string }[] = [
  { value: 'TRANSFER', label: 'Transfer' },
  { value: 'CASH', label: 'Tunai' },
  { value: 'GIRO', label: 'Giro' },
];

export function paymentMethodLabel(value: SupplierPaymentMethod): string {
  return PAYMENT_METHODS.find((m) => m.value === value)?.label ?? value;
}

export const DEBT_FILTERS: { value: DebtFilter; label: string; icon: string }[] = [
  { value: 'OPEN', label: 'Belum lunas', icon: 'pi pi-wallet' },
  { value: 'OVERDUE', label: 'Lewat jatuh tempo', icon: 'pi pi-exclamation-triangle' },
  { value: 'DUE_SOON', label: 'Jatuh tempo ≤ 7 hari', icon: 'pi pi-clock' },
  { value: 'PAID', label: 'Lunas', icon: 'pi pi-check-circle' },
  { value: 'ALL', label: 'Semua', icon: 'pi pi-list' },
];

/** "−3" → "Lewat 3 hari", "0" → "Hari ini", "5" → "5 hari lagi". */
export function dueLabel(daysLeft: number): string {
  if (daysLeft < 0) return `Lewat ${-daysLeft} hari`;
  if (daysLeft === 0) return 'Hari ini';
  return `${daysLeft} hari lagi`;
}

/** Tanggal ISO + n hari (lokal). */
export function addDays(iso: string, days: number): string {
  const d = new Date(`${iso}T00:00:00`);
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
