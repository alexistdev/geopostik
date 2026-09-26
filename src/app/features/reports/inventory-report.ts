import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SkeletonModule } from 'primeng/skeleton';

import type { InventoryReport as InventoryReportData } from '../../bindings/InventoryReport';
import { reportApi } from '../../core/api/report.api';
import { Notify } from '../../core/ui/notify';
import { RupiahPipe } from '../../shared/format';
import { ReportPrint } from './report-print';

/** Tab Laporan › Nilai Persediaan: Σ stok × HPP batch, per kategori dan per obat (hak VIEW_COST). */
@Component({
  selector: 'app-inventory-report',
  imports: [FormsModule, ButtonModule, IconFieldModule, InputIconModule, InputTextModule, SkeletonModule, RupiahPipe],
  templateUrl: './inventory-report.html',
  styleUrl: './inventory-report.scss',
})
export class InventoryReport {
  private readonly notify = inject(Notify);
  private readonly printer = inject(ReportPrint);
  private readonly content = viewChild<ElementRef<HTMLElement>>('content');

  protected readonly data = signal<InventoryReportData | null>(null);
  protected readonly loading = signal(false);
  protected readonly q = signal('');

  protected readonly rows = computed(() => {
    const q = this.q().trim().toLowerCase();
    const rows = this.data()?.rows ?? [];
    return q ? rows.filter((r) => r.name.toLowerCase().includes(q) || r.code.toLowerCase().includes(q)) : rows;
  });

  protected readonly categoryMax = computed(() => Math.max(1, ...(this.data()?.byCategory ?? []).map((c) => c.value)));

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      this.data.set(await reportApi.inventory());
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected print(): void {
    const el = this.content()?.nativeElement;
    if (!this.data() || !el) return;
    this.printer.print('Laporan Nilai Persediaan', `Per ${new Date().toLocaleDateString('id-ID')}`, el);
  }

  protected share(value: number): string {
    const total = this.data()?.totalValue ?? 0;
    return total > 0 ? `${((value / total) * 100).toFixed(1)}%` : '–';
  }
}
