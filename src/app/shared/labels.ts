import type { DrugClass } from '../bindings/DrugClass';
import type { PriceMode } from '../bindings/PriceMode';
import type { Role } from '../bindings/Role';

type TagSeverity = 'success' | 'info' | 'warn' | 'danger' | 'secondary' | 'contrast';

export const DRUG_CLASSES: { value: DrugClass; label: string; severity: TagSeverity }[] = [
  { value: 'FREE', label: 'Bebas', severity: 'success' },
  { value: 'LIMITED_FREE', label: 'Bebas Terbatas', severity: 'info' },
  { value: 'HARD', label: 'Keras', severity: 'danger' },
  { value: 'PSYCHOTROPIC', label: 'Psikotropika', severity: 'warn' },
  { value: 'NARCOTIC', label: 'Narkotika', severity: 'contrast' },
];

export function drugClassInfo(value: DrugClass) {
  return DRUG_CLASSES.find((d) => d.value === value)!;
}

export const PRICE_MODES: { value: PriceMode; label: string }[] = [
  { value: 'AUTO', label: 'Otomatis' },
  { value: 'MANUAL', label: 'Manual' },
];

/** Peran pengguna, urut dari kewenangan tertinggi (sama dengan urutan di Rust). */
export const ROLES: { value: Role; label: string; severity: TagSeverity; hint: string }[] = [
  { value: 'OWNER', label: 'Pemilik', severity: 'contrast', hint: 'Pengguna, pengaturan, backup, log, harga & HPP' },
  { value: 'PHARMACIST', label: 'Apoteker', severity: 'success', hint: 'Obat keras, validasi resep, void, opname, pemusnahan, penjualan (tanpa buka/tutup shift)' },
  { value: 'TECHNICIAN', label: 'TTK', severity: 'info', hint: 'Input resep, data obat, penerimaan barang, hitung opname, penjualan (tanpa buka/tutup shift)' },
  { value: 'CASHIER', label: 'Kasir', severity: 'secondary', hint: 'Penjualan bebas, buka/tutup shift' },
];

export function roleInfo(value: Role) {
  return ROLES.find((r) => r.value === value)!;
}
