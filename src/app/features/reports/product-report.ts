import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { ButtonModule } from 'primeng/button';
import { SkeletonModule } from 'primeng/skeleton';

import type { ProductReport as ProductReportData } from '../../bindings/ProductReport';
import { reportApi } from '../../core/api/report.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe } from '../../shared/format';
import { formatDate } from '../inventory/stock-format';
import { ReportPeriod, thisMonth } from './report-period';
import { ReportPrint } from './report-print';

type Sort = 'amount' | 'qty' | 'profit';

/** Tab Laporan › Obat Terlaris: obat terjual per periode dan obat tidak laku (slow moving). */
@Component({
  selector: 'app-product-report',
  imports: [ButtonModule, SkeletonModule, ReportPeriod, RupiahPipe, DateTimePipe],
  templateUrl: './product-report.html',
  styleUrl: './product-report.scss',
})
export class ProductReport {
  private readonly notify = inject(Notify);
  private readonly printer = inject(ReportPrint);
  private readonly content = viewChild<ElementRef<HTMLElement>>('content');

  protected readonly from = signal(thisMonth().from);
  protected readonly to = signal(thisMonth().to);
  protected readonly data = signal<ProductReportData | null>(null);
  protected readonly loading = signal(false);
  protected readonly sort = signal<Sort>('amount');

  protected readonly formatDate = formatDate;

  protected readonly viewCost = signal(inject(AuthService).can('VIEW_COST'));

  protected readonly top = computed(() => {
    const rows = [...(this.data()?.top ?? [])];
    const key = this.sort();
    const val = (r: (typeof rows)[number]) => (key === 'qty' ? r.qtyBase : key === 'profit' ? (r.grossProfit ?? 0) : r.amount);
    return rows.sort((a, b) => val(b) - val(a));
  });

  protected readonly topMax = computed(() => Math.max(1, ...this.top().map((r) => r.amount)));
  protected readonly topTotal = computed(() => this.top().reduce((s, r) => s + r.amount, 0));
  protected readonly slowValue = computed(() => (this.data()?.slow ?? []).reduce((s, r) => s + (r.value ?? 0), 0));

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      this.data.set(await reportApi.products({ from: this.from(), to: this.to() }));
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
    this.printer.print('Laporan Obat Terlaris & Tidak Laku', `Periode ${formatDate(d.from)} – ${formatDate(d.to)}`, el);
  }

  /** Margin laba kotor terhadap nilai jual, dalam persen. */
  protected margin(amount: number, profit: number | null): string {
    return profit == null || amount <= 0 ? '–' : `${((profit / amount) * 100).toFixed(1)}%`;
  }
}
