import { Pipe, PipeTransform } from '@angular/core';

const rupiahFormat = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 0 });
const decimalFormat = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 2 });

/** 15000 → "Rp15.000". Uang selalu bilangan bulat rupiah. */
export function formatRupiah(value: number | null | undefined): string {
  return value == null ? '–' : `Rp${rupiahFormat.format(value)}`;
}

/** HPP ×100 → "Rp150,5". */
export function formatCostX100(value: number | null | undefined): string {
  return value == null ? '–' : `Rp${decimalFormat.format(value / 100)}`;
}

/** Basis point → persen: 2500 → 25. */
export function bpToPercent(bp: number | null | undefined): number | null {
  return bp == null ? null : bp / 100;
}

/** Persen → basis point: 12.5 → 1250. */
export function percentToBp(percent: number | null | undefined): number | null {
  return percent == null ? null : Math.round(percent * 100);
}

@Pipe({ name: 'rupiah' })
export class RupiahPipe implements PipeTransform {
  transform(value: number | null | undefined): string {
    return formatRupiah(value);
  }
}

@Pipe({ name: 'costX100' })
export class CostX100Pipe implements PipeTransform {
  transform(value: number | null | undefined): string {
    return formatCostX100(value);
  }
}
