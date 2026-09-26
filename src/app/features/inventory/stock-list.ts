import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { SelectButtonModule } from 'primeng/selectbutton';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';
import { TooltipModule } from 'primeng/tooltip';

import type { BatchRow } from '../../bindings/BatchRow';
import type { Category } from '../../bindings/Category';
import type { NamedItem } from '../../bindings/NamedItem';
import type { ProductBatches } from '../../bindings/ProductBatches';
import type { StockFilter } from '../../bindings/StockFilter';
import type { StockRow } from '../../bindings/StockRow';
import { inventoryApi } from '../../core/api/inventory.api';
import { masterApi } from '../../core/api/master.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { CostX100Pipe, RupiahPipe } from '../../shared/format';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master/master-table';
import { STOCK_FILTERS, expiryState, formatDate, isoDate } from './stock-format';

const SOURCE_LABELS: Record<BatchRow['sourceType'], string> = {
  OPENING: 'Stok awal',
  PURCHASE: 'Pembelian',
  ADJUSTMENT: 'Opname',
};

/** Tab Stok: stok per obat, dirinci per batch (FEFO) dengan status ED dan kunci batch. */
@Component({
  selector: 'app-stock-list',
  imports: [
    FormsModule,
    ButtonModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    SelectModule,
    SelectButtonModule,
    TableModule,
    TagModule,
    ToggleSwitchModule,
    TooltipModule,
    RupiahPipe,
    CostX100Pipe,
  ],
  templateUrl: './stock-list.html',
  styleUrl: './stock-list.scss',
})
export class StockList {
  private readonly notify = inject(Notify);
  private readonly router = inject(Router);
  private readonly auth = inject(AuthService);
  protected readonly canLock = this.auth.can('STOCK_COUNT_APPROVE');
  protected readonly viewCost = this.auth.can('VIEW_COST');

  protected readonly filters = STOCK_FILTERS;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected readonly sourceLabels = SOURCE_LABELS;
  protected readonly formatDate = formatDate;
  protected pageSize = MASTER_PAGE_SIZES[0];

  protected readonly rows = signal<StockRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);
  protected readonly today = signal(isoDate(new Date()));
  protected readonly categories = signal<Category[]>([]);
  protected readonly racks = signal<NamedItem[]>([]);

  protected q = '';
  protected filter: StockFilter = 'ALL';
  protected categoryId: number | null = null;
  protected rackId: number | null = null;
  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  /** Baris yang dibuka beserta batch-nya. */
  protected expanded: Record<number, boolean> = {};
  protected readonly batches = signal<Record<number, ProductBatches>>({});
  protected includeEmpty = false;

  /** Dialog kunci batch. */
  protected locking: { batch: BatchRow; productId: number; productName: string; reason: string } | null = null;
  protected readonly lockSaving = signal(false);

  constructor() {
    masterApi.categoryList().then(
      (c) => this.categories.set(c.filter((x) => x.isActive)),
      (e) => this.notify.error(e),
    );
    masterApi.rackList().then(
      (r) => this.racks.set(r.filter((x) => x.isActive)),
      (e) => this.notify.error(e),
    );
  }

  protected async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await inventoryApi.stockList({
        q: this.q.trim() || null,
        categoryId: this.categoryId,
        rackId: this.rackId,
        filter: this.filter,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
      this.today.set(page.today);
      // Batch yang sedang dibuka ikut dimuat ulang agar tetap sesuai.
      for (const id of Object.keys(this.expanded).map(Number)) {
        if (page.rows.some((r) => r.productId === id)) this.loadBatches(id);
      }
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

  protected rowExpanded(row: StockRow): void {
    this.loadBatches(row.productId);
  }

  protected async loadBatches(productId: number): Promise<void> {
    try {
      const b = await inventoryApi.stockBatches(productId, this.includeEmpty);
      this.batches.update((all) => ({ ...all, [productId]: b }));
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected includeEmptyChanged(productId: number): void {
    this.loadBatches(productId);
  }

  protected isLow(row: StockRow): boolean {
    return row.minStockBase > 0 && row.stockBase < row.minStockBase;
  }

  protected expiry(date: string | null) {
    return expiryState(date, this.today());
  }

  protected openCard(productId: number, batchId?: number): void {
    this.router.navigate(['/stok/kartu'], { queryParams: { produk: productId, batch: batchId } });
  }

  protected askLock(row: StockRow, batch: BatchRow): void {
    if (batch.isLocked) {
      this.setLocked(row.productId, batch, false, null);
    } else {
      this.locking = { batch, productId: row.productId, productName: row.name, reason: '' };
    }
  }

  protected async confirmLock(): Promise<void> {
    const l = this.locking;
    if (!l) return;
    this.lockSaving.set(true);
    const ok = await this.setLocked(l.productId, l.batch, true, l.reason);
    this.lockSaving.set(false);
    if (ok) this.locking = null;
  }

  private async setLocked(productId: number, batch: BatchRow, locked: boolean, reason: string | null): Promise<boolean> {
    try {
      await inventoryApi.batchSetLocked(batch.id, locked, reason);
      this.notify.success(`Batch ${batch.batchNumber} ${locked ? 'dikunci' : 'dibuka kuncinya'}`);
      await this.load();
      return true;
    } catch (e) {
      this.notify.error(e);
      return false;
    }
  }
}
