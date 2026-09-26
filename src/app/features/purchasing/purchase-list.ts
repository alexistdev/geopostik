import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { PurchaseRow } from '../../bindings/PurchaseRow';
import type { PurchaseStatus } from '../../bindings/PurchaseStatus';
import type { Supplier } from '../../bindings/Supplier';
import { purchasingApi } from '../../core/api/purchasing.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe } from '../../shared/format';
import { formatDate } from '../inventory/stock-format';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master/master-table';
import { PURCHASE_STATUSES, purchaseStatusInfo } from './purchase-labels';

/** Tab Penerimaan Barang: daftar faktur pembelian. */
@Component({
  selector: 'app-purchase-list',
  imports: [
    FormsModule,
    ButtonModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    SelectModule,
    TableModule,
    TagModule,
    DateTimePipe,
    RupiahPipe,
  ],
  templateUrl: './purchase-list.html',
  styleUrl: './purchase-list.scss',
})
export class PurchaseList {
  private readonly notify = inject(Notify);
  private readonly router = inject(Router);
  protected readonly canDebt = inject(AuthService).can('SUPPLIER_DEBT_MANAGE');

  protected readonly statuses = PURCHASE_STATUSES;
  protected readonly statusInfo = purchaseStatusInfo;
  protected readonly formatDate = formatDate;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected pageSize = MASTER_PAGE_SIZES[0];

  protected readonly rows = signal<PurchaseRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);
  protected readonly suppliers = signal<Supplier[]>([]);

  protected q = '';
  protected status: PurchaseStatus | null = null;
  protected supplierId: number | null = null;
  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    purchasingApi.supplierList().then((s) => this.suppliers.set(s), (e) => this.notify.error(e));
  }

  protected async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await purchasingApi.page({
        q: this.q.trim() || null,
        status: this.status,
        supplierId: this.supplierId,
        dateFrom: null,
        dateTo: null,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
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

  protected searchChanged(): void {
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.load(0), 250);
  }

  protected open(id: number | 'baru'): void {
    this.router.navigate(['/pembelian/faktur', id]);
  }
}
