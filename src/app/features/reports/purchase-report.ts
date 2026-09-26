import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { RouterLink } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { SkeletonModule } from 'primeng/skeleton';

import type { PurchaseReport as PurchaseReportData } from '../../bindings/PurchaseReport';
import { reportApi } from '../../core/api/report.api';
import { Notify } from '../../core/ui/notify';
import { RupiahPipe } from '../../shared/format';
import { formatDate } from '../inventory/stock-format';
import { ReportPeriod, thisMonth } from './report-period';
import { ReportPrint } from './report-print';

/** Tab Laporan › Pembelian: faktur yang diposting per periode (tanggal terima), per supplier. */
@Component({
  selector: 'app-purchase-report',
  imports: [RouterLink, ButtonModule, SkeletonModule, ReportPeriod, RupiahPipe],
  templateUrl: './purchase-report.html',
  styleUrl: './purchase-report.scss',
})
export class PurchaseReport {
  private readonly notify = inject(Notify);
  private readonly printer = inject(ReportPrint);
  private readonly content = viewChild<ElementRef<HTMLElement>>('content');

  protected readonly from = signal(thisMonth().from);
  protected readonly to = signal(thisMonth().to);
  protected readonly data = signal<PurchaseReportData | null>(null);
  protected readonly loading = signal(false);

  protected readonly formatDate = formatDate;
  protected readonly supplierMax = computed(() => Math.max(1, ...(this.data()?.bySupplier ?? []).map((s) => s.total)));
  protected readonly outstanding = computed(() =>
    (this.data()?.invoices ?? []).reduce((s, i) => s + (i.outstanding ?? 0), 0),
  );

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      this.data.set(await reportApi.purchases({ from: this.from(), to: this.to() }));
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
    this.printer.print('Laporan Pembelian', `Tanggal terima ${formatDate(d.from)} – ${formatDate(d.to)}`, el);
  }
}
