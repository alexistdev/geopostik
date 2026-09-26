import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { TooltipModule } from 'primeng/tooltip';

import type { PrescriptionRow } from '../../bindings/PrescriptionRow';
import type { PrescriptionStatus } from '../../bindings/PrescriptionStatus';
import { prescriptionApi } from '../../core/api/prescription.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe } from '../../shared/format';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master/master-table';
import { PrescriptionDialog } from './prescription-dialog';
import { PRESCRIPTION_STATUSES, formatDate, statusInfo } from './rx-labels';

/** Menu Resep: daftar resep per status, input resep baru, skrining, dan batal. */
@Component({
  selector: 'app-prescription-list',
  imports: [
    FormsModule,
    ButtonModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    TableModule,
    TagModule,
    TooltipModule,
    DateTimePipe,
    RupiahPipe,
    PrescriptionDialog,
  ],
  templateUrl: './prescription-list.html',
  styleUrl: './prescription-list.scss',
})
export class PrescriptionList {
  private readonly notify = inject(Notify);
  protected readonly canValidate = inject(AuthService).can('PRESCRIPTION_VALIDATE');

  protected readonly statuses = PRESCRIPTION_STATUSES;
  protected readonly statusInfo = statusInfo;
  protected readonly formatDate = formatDate;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;

  protected readonly rows = signal<PrescriptionRow[]>([]);
  protected readonly total = signal(0);
  protected readonly draftCount = signal(0);
  protected readonly screenedCount = signal(0);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);
  protected pageSize = MASTER_PAGE_SIZES[0];

  /** `null` = semua status. Apoteker langsung melihat resep yang menunggu skrining. */
  protected readonly status = signal<PrescriptionStatus | null>(this.canValidate ? 'DRAFT' : null);
  protected q = '';
  protected dateFrom = '';
  protected dateTo = '';
  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  /** `undefined` = dialog tertutup, `null` = resep baru. */
  protected readonly editing = signal<number | null | undefined>(undefined);

  constructor() {
    this.load(0);
  }

  protected async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await prescriptionApi.page({
        q: this.q.trim() || null,
        status: this.status(),
        dateFrom: this.dateFrom || null,
        dateTo: this.dateTo || null,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
      this.draftCount.set(page.draftCount);
      this.screenedCount.set(page.screenedCount);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected pageChanged(first: number, rows: number): void {
    this.pageSize = rows;
    this.load(first);
  }

  protected setStatus(status: PrescriptionStatus | null): void {
    this.status.set(status);
    this.load(0);
  }

  protected searchChanged(): void {
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.load(0), 250);
  }

  protected countOf(status: PrescriptionStatus): number | null {
    if (status === 'DRAFT') return this.draftCount();
    if (status === 'SCREENED') return this.screenedCount();
    return null;
  }

  protected closed(changed: boolean): void {
    this.editing.set(undefined);
    if (changed) this.load();
  }
}
