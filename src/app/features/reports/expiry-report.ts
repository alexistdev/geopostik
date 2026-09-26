import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { SelectButtonModule } from 'primeng/selectbutton';
import { SkeletonModule } from 'primeng/skeleton';

import type { ExpiryReport as ExpiryReportData } from '../../bindings/ExpiryReport';
import { reportApi } from '../../core/api/report.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { RupiahPipe } from '../../shared/format';
import { NEAR_EXPIRY_DAYS, formatDate } from '../inventory/stock-format';
import { ReportPrint } from './report-print';

type Show = 'ALL' | 'EXPIRED' | 'NEAR';

const HORIZONS = [
  { label: '1 bulan', value: 30 },
  { label: '3 bulan', value: NEAR_EXPIRY_DAYS },
  { label: '6 bulan', value: 180 },
  { label: '1 tahun', value: 365 },
];

/** Tab Laporan › Kedaluwarsa: batch ber-stok yang sudah ED dan akan ED dalam jangka tertentu. */
@Component({
  selector: 'app-expiry-report',
  imports: [FormsModule, ButtonModule, SelectButtonModule, SkeletonModule, RupiahPipe],
  templateUrl: './expiry-report.html',
  styleUrl: './expiry-report.scss',
})
export class ExpiryReport {
  private readonly notify = inject(Notify);
  private readonly printer = inject(ReportPrint);
  private readonly content = viewChild<ElementRef<HTMLElement>>('content');

  protected readonly horizons = HORIZONS;
  protected readonly viewCost = inject(AuthService).can('VIEW_COST');
  protected readonly formatDate = formatDate;

  protected days = NEAR_EXPIRY_DAYS;
  protected readonly show = signal<Show>('ALL');
  protected readonly data = signal<ExpiryReportData | null>(null);
  protected readonly loading = signal(false);

  protected readonly rows = computed(() => {
    const rows = this.data()?.rows ?? [];
    const s = this.show();
    return s === 'ALL' ? rows : rows.filter((r) => (s === 'EXPIRED' ? r.daysLeft <= 0 : r.daysLeft > 0));
  });

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      this.data.set(await reportApi.expiry({ days: this.days }));
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected print(): void {
    const d = this.data();
    const el = this.content()?.nativeElement;
    if (!d || !el) return;
    const label = HORIZONS.find((h) => h.value === this.days)?.label ?? `${this.days} hari`;
    this.printer.print('Laporan Obat Kedaluwarsa', `Per ${formatDate(d.today)} · ED sampai ${label} ke depan`, el);
  }

  protected daysLabel(days: number): string {
    if (days < 0) return `lewat ${-days} hari`;
    if (days === 0) return 'ED hari ini';
    return `${days} hari lagi`;
  }
}
