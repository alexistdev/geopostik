import type { CompoundForm } from '../../bindings/CompoundForm';
import type { Gender } from '../../bindings/Gender';
import type { PrescriptionStatus } from '../../bindings/PrescriptionStatus';

type TagSeverity = 'success' | 'info' | 'warn' | 'danger' | 'secondary' | 'contrast';

export const PRESCRIPTION_STATUSES: { value: PrescriptionStatus; label: string; severity: TagSeverity; icon: string }[] = [
  { value: 'DRAFT', label: 'Menunggu skrining', severity: 'warn', icon: 'pi pi-hourglass' },
  { value: 'SCREENED', label: 'Siap dibayar', severity: 'info', icon: 'pi pi-verified' },
  { value: 'PAID', label: 'Dibayar', severity: 'success', icon: 'pi pi-check-circle' },
  { value: 'CANCELLED', label: 'Batal', severity: 'secondary', icon: 'pi pi-times-circle' },
];

export function statusInfo(value: PrescriptionStatus) {
  return PRESCRIPTION_STATUSES.find((s) => s.value === value)!;
}

/** Bentuk sediaan racikan beserta satuan hitungnya. */
export const COMPOUND_FORMS: { value: CompoundForm; label: string; unit: string }[] = [
  { value: 'POWDER', label: 'Puyer', unit: 'bungkus' },
  { value: 'CAPSULE', label: 'Kapsul', unit: 'kapsul' },
  { value: 'OINTMENT', label: 'Salep / krim', unit: 'pot' },
  { value: 'LIQUID', label: 'Sirup / cairan', unit: 'botol' },
  { value: 'OTHER', label: 'Lainnya', unit: 'buah' },
];

export function compoundFormInfo(value: CompoundForm | null | undefined) {
  return COMPOUND_FORMS.find((f) => f.value === value) ?? COMPOUND_FORMS[COMPOUND_FORMS.length - 1];
}

export const GENDERS: { value: Gender; label: string }[] = [
  { value: 'M', label: 'Laki-laki' },
  { value: 'F', label: 'Perempuan' },
];

export function genderLabel(value: Gender | null | undefined): string {
  return GENDERS.find((g) => g.value === value)?.label ?? '–';
}

/** Tanggal hari ini `YYYY-MM-DD` (waktu lokal). */
export function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function dateToIso(d: Date | null | undefined): string | null {
  return d ? `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` : null;
}

export function isoToDate(value: string | null | undefined): Date | null {
  const m = value?.match(/^(\d{4})-(\d{2})-(\d{2})/);
  return m ? new Date(+m[1], +m[2] - 1, +m[3]) : null;
}

/** "2026-09-26" → "26/09/2026". */
export function formatDate(value: string | null | undefined): string {
  const m = value?.match(/^(\d{4})-(\d{2})-(\d{2})/);
  return m ? `${m[3]}/${m[2]}/${m[1]}` : '–';
}

/** Umur dari tanggal lahir, untuk dicetak di resep/etiket: "7 th", "8 bln", "12 hr". */
export function ageFromBirthDate(birth: string | null | undefined, at = new Date()): string | null {
  const b = isoToDate(birth);
  if (!b || b > at) return null;
  let months = (at.getFullYear() - b.getFullYear()) * 12 + (at.getMonth() - b.getMonth());
  if (at.getDate() < b.getDate()) months--;
  if (months >= 24) return `${Math.floor(months / 12)} th`;
  if (months >= 1) return `${months} bln`;
  const days = Math.floor((at.getTime() - b.getTime()) / 86_400_000);
  return `${days} hr`;
}

function pad(n: number): string {
  return String(n).padStart(2, '0');
}
