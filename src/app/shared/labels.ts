import type { DrugClass } from '../bindings/DrugClass';
import type { PriceMode } from '../bindings/PriceMode';

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
