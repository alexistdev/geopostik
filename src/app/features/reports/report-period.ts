import { Component, model, output } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';

import type { ReportRange } from '../../bindings/ReportRange';

/** Date lokal → `YYYY-MM-DD` (tanpa geser zona waktu). */
export function ymd(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** `YYYY-MM-DD` → Date lokal. */
export function localDate(value: string): Date {
  const [y, m, d] = value.split('-').map(Number);
  return new Date(y, m - 1, d);
}

interface Preset {
  label: string;
  range: () => ReportRange;
}

function shift(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  return ymd(d);
}

const PRESETS: Preset[] = [
  { label: 'Hari ini', range: () => ({ from: shift(0), to: shift(0) }) },
  { label: 'Kemarin', range: () => ({ from: shift(-1), to: shift(-1) }) },
  { label: '7 hari', range: () => ({ from: shift(-6), to: shift(0) }) },
  {
    label: 'Bulan ini',
    range: () => {
      const d = new Date();
      return { from: ymd(new Date(d.getFullYear(), d.getMonth(), 1)), to: ymd(d) };
    },
  },
  {
    label: 'Bulan lalu',
    range: () => {
      const d = new Date();
      return {
        from: ymd(new Date(d.getFullYear(), d.getMonth() - 1, 1)),
        to: ymd(new Date(d.getFullYear(), d.getMonth(), 0)),
      };
    },
  },
];

/** Rentang awal laporan: bulan berjalan. */
export function thisMonth(): ReportRange {
  return PRESETS[3].range();
}

/** Pemilih periode laporan: tanggal awal–akhir (inklusif) dan pintasan umum. */
@Component({
  selector: 'app-report-period',
  imports: [FormsModule, ButtonModule, InputTextModule],
  template: `
    <div class="period">
      <div class="dates">
        <input pInputText type="date" aria-label="Dari tanggal" [ngModel]="from()" (ngModelChange)="from.set($event)" [max]="to()" />
        <span>s/d</span>
        <input pInputText type="date" aria-label="Sampai tanggal" [ngModel]="to()" (ngModelChange)="to.set($event)" [min]="from()" />
        <p-button label="Tampilkan" icon="pi pi-search" size="small" (onClick)="apply.emit()" />
      </div>
      <div class="presets">
        @for (p of presets; track p.label) {
          <button type="button" [class.active]="isActive(p)" (click)="pick(p)">{{ p.label }}</button>
        }
      </div>
    </div>
  `,
  styles: `
    .period {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem 1.25rem;
    }
    .dates {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      span {
        font-size: 0.85rem;
        color: var(--p-text-muted-color);
      }
      input {
        width: 10.5rem;
      }
    }
    .presets {
      display: flex;
      flex-wrap: wrap;
      gap: 0.35rem;
      button {
        padding: 0.3rem 0.75rem;
        font: inherit;
        font-size: 0.8rem;
        font-weight: 600;
        color: var(--p-text-muted-color);
        background: transparent;
        border: 1px solid var(--p-content-border-color);
        border-radius: 999px;
        cursor: pointer;
        &:hover {
          border-color: var(--p-primary-400);
          color: var(--p-primary-700);
        }
        &.active {
          color: #fff;
          background: var(--p-primary-700);
          border-color: var(--p-primary-700);
        }
      }
    }
  `,
})
export class ReportPeriod {
  readonly from = model.required<string>();
  readonly to = model.required<string>();
  /** Dipanggil saat user menekan Tampilkan atau memilih pintasan. */
  readonly apply = output<void>();

  protected readonly presets = PRESETS;

  protected isActive(p: Preset): boolean {
    const r = p.range();
    return r.from === this.from() && r.to === this.to();
  }

  protected pick(p: Preset): void {
    const r = p.range();
    this.from.set(r.from);
    this.to.set(r.to);
    this.apply.emit();
  }
}
