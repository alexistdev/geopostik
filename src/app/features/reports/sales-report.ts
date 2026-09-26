import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { ButtonModule } from 'primeng/button';
import { SkeletonModule } from 'primeng/skeleton';

import type { SalesReport as SalesReportData } from '../../bindings/SalesReport';
import { reportApi } from '../../core/api/report.api';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe, formatRupiah } from '../../shared/format';
import { formatDate } from '../inventory/stock-format';
import { ReportPeriod, localDate, thisMonth } from './report-period';
import { ReportPrint } from './report-print';

const METHOD_LABELS: Record<string, string> = { CASH: 'Tunai', QRIS: 'QRIS', DEBIT: 'Debit' };
const DAY_SHORT = new Intl.DateTimeFormat('id-ID', { weekday: 'short', day: 'numeric', month: 'short' });
const COMPACT = new Intl.NumberFormat('id-ID', { notation: 'compact', maximumFractionDigits: 1 });

/** Tab Laporan › Penjualan: omzet per periode, per hari, metode bayar, kasir, dan shift. */
@Component({
  selector: 'app-sales-report',
  imports: [ButtonModule, SkeletonModule, ReportPeriod, RupiahPipe, DateTimePipe],
  templateUrl: './sales-report.html',
  styleUrl: './sales-report.scss',
})
export class SalesReport {
  private readonly notify = inject(Notify);
  private readonly printer = inject(ReportPrint);
  private readonly content = viewChild<ElementRef<HTMLElement>>('content');

  protected readonly from = signal(thisMonth().from);
  protected readonly to = signal(thisMonth().to);
  protected readonly data = signal<SalesReportData | null>(null);
  protected readonly loading = signal(false);
  protected readonly hover = signal<number | null>(null);

  protected readonly methodLabels = METHOD_LABELS;
  protected readonly formatDate = formatDate;

  protected readonly average = computed(() => {
    const s = this.data()?.summary;
    return s && s.count > 0 ? Math.round(s.net / s.count) : 0;
  });

  /** Margin laba kotor terhadap penjualan tanpa PPN, dalam persen. */
  protected readonly margin = computed(() => {
    const s = this.data()?.summary;
    if (!s || s.grossProfit == null) return null;
    const base = s.net - s.tax;
    return base > 0 ? Math.round((s.grossProfit / base) * 1000) / 10 : null;
  });

  protected readonly bars = computed(() => {
    const daily = this.data()?.daily ?? [];
    const max = Math.max(1, ...daily.map((d) => d.amount));
    return daily.map((d) => ({
      ...d,
      label: DAY_SHORT.format(localDate(d.date)),
      pct: (d.amount / max) * 100,
      short: d.amount > 0 ? COMPACT.format(d.amount) : '',
    }));
  });

  /** Label sumbu hanya di sebagian batang bila rentangnya panjang. */
  protected readonly labelEvery = computed(() => Math.max(1, Math.ceil(this.bars().length / 16)));

  protected readonly activeDays = computed(() => (this.data()?.daily ?? []).filter((d) => d.count > 0));
  protected readonly methodTotal = computed(() => (this.data()?.byMethod ?? []).reduce((s, m) => s + m.amount, 0));
  protected readonly cashierMax = computed(() => Math.max(1, ...(this.data()?.byCashier ?? []).map((c) => c.amount)));

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      this.data.set(await reportApi.sales({ from: this.from(), to: this.to() }));
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
    this.printer.print(
      d.ownOnly ? 'Laporan Penjualan Saya' : 'Laporan Penjualan',
      `Periode ${formatDate(d.from)} – ${formatDate(d.to)}`,
      el,
    );
  }

  protected barTitle(b: { label: string; amount: number; count: number }): string {
    return `${b.label}: ${formatRupiah(b.amount)} (${b.count} nota)`;
  }
}
