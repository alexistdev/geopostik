import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { SkeletonModule } from 'primeng/skeleton';

import type { SipnapReport as SipnapReportData } from '../../bindings/SipnapReport';
import type { SipnapRow } from '../../bindings/SipnapRow';
import { reportApi } from '../../core/api/report.api';
import { Notify } from '../../core/ui/notify';
import { ReportPrint } from './report-print';

const MONTH_LONG = new Intl.DateTimeFormat('id-ID', { month: 'long', year: 'numeric' });

const CLASSES = [
  { value: 'NARCOTIC', label: 'Narkotika' },
  { value: 'PSYCHOTROPIC', label: 'Psikotropika' },
];

/** `YYYY-MM` bulan lalu: laporan SIPNAP dikirim untuk bulan yang sudah berakhir. */
function lastMonth(): string {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() - 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
}

/**
 * Tab Laporan › Narkotika & Psikotropika: mutasi sebulan per obat dari kartu stok, bahan pengisian
 * SIPNAP. Semua jumlah dalam satuan dasar.
 */
@Component({
  selector: 'app-sipnap-report',
  imports: [FormsModule, ButtonModule, InputTextModule, SkeletonModule],
  templateUrl: './sipnap-report.html',
  styleUrl: './sipnap-report.scss',
})
export class SipnapReport {
  private readonly notify = inject(Notify);
  private readonly printer = inject(ReportPrint);
  private readonly content = viewChild<ElementRef<HTMLElement>>('content');

  protected month = lastMonth();
  protected readonly data = signal<SipnapReportData | null>(null);
  protected readonly loading = signal(false);

  protected readonly title = computed(() => {
    const d = this.data();
    return d ? MONTH_LONG.format(new Date(d.year, d.month - 1, 1)) : '';
  });

  protected readonly groups = computed(() => {
    const rows = this.data()?.rows ?? [];
    return CLASSES.map((c) => ({ ...c, rows: rows.filter((r) => r.drugClass === c.value) }));
  });

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    const [year, month] = this.month.split('-').map(Number);
    if (!year || !month) return;
    this.loading.set(true);
    try {
      this.data.set(await reportApi.sipnap({ year, month }));
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected print(): void {
    const el = this.content()?.nativeElement;
    if (!this.data() || !el) return;
    this.printer.print('Laporan Narkotika & Psikotropika', `Periode ${this.title()}`, el);
  }

  protected hasMovement(r: SipnapRow): boolean {
    return r.received !== 0 || r.sold !== 0 || r.destroyed !== 0 || r.adjusted !== 0;
  }
}
