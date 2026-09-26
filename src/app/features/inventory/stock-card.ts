import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { AutoCompleteModule } from 'primeng/autocomplete';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { BatchRow } from '../../bindings/BatchRow';
import type { ProductBatches } from '../../bindings/ProductBatches';
import type { ProductListRow } from '../../bindings/ProductListRow';
import type { StockCardPage } from '../../bindings/StockCardPage';
import type { StockCardRow } from '../../bindings/StockCardRow';
import { inventoryApi } from '../../core/api/inventory.api';
import { masterApi } from '../../core/api/master.api';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe } from '../../shared/format';
import { MOVEMENT_TYPES, formatDate } from './stock-format';

const PAGE_SIZES = [50, 100, 200];

/** Tab Kartu Stok: riwayat keluar-masuk satu obat (atau satu batch) dengan saldo berjalan. */
@Component({
  selector: 'app-stock-card',
  imports: [FormsModule, AutoCompleteModule, ButtonModule, InputTextModule, SelectModule, TableModule, TagModule, DateTimePipe],
  templateUrl: './stock-card.html',
  styleUrl: './stock-card.scss',
})
export class StockCard {
  private readonly notify = inject(Notify);
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);

  protected readonly movementTypes = MOVEMENT_TYPES;
  protected readonly formatDate = formatDate;
  protected readonly pageSizes = PAGE_SIZES;
  protected pageSize = PAGE_SIZES[0];

  protected product: ProductListRow | null = null;
  protected readonly suggestions = signal<ProductListRow[]>([]);
  protected readonly info = signal<ProductBatches | null>(null);
  protected batchId: number | null = null;
  protected dateFrom = '';
  protected dateTo = '';

  protected readonly page = signal<StockCardPage | null>(null);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);

  constructor() {
    const params = this.route.snapshot.queryParamMap;
    const productId = Number(params.get('produk'));
    if (productId) {
      this.batchId = Number(params.get('batch')) || null;
      this.selectProductId(productId);
    }
  }

  protected async search(query: string): Promise<void> {
    try {
      const r = await masterApi.productList({
        q: query.trim() || null,
        categoryId: null,
        drugClass: null,
        includeInactive: true,
        offset: 0,
        limit: 20,
      });
      this.suggestions.set(r.rows);
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected productSelected(p: ProductListRow): void {
    this.batchId = null;
    this.selectProductId(p.id);
  }

  private async selectProductId(id: number): Promise<void> {
    try {
      const info = await inventoryApi.stockBatches(id, true);
      this.info.set(info);
      this.product ??= { id: info.productId, code: info.code, name: info.name } as ProductListRow;
      this.syncUrl();
      await this.load(0);
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected filterChanged(): void {
    this.syncUrl();
    this.load(0);
  }

  protected clear(): void {
    this.product = null;
    this.info.set(null);
    this.page.set(null);
    this.batchId = null;
    this.syncUrl();
  }

  private syncUrl(): void {
    this.router.navigate([], {
      relativeTo: this.route,
      queryParams: { produk: this.info()?.productId ?? null, batch: this.batchId },
      replaceUrl: true,
    });
  }

  protected pageChanged(first: number, rows: number): void {
    if (!this.info()) return;
    this.pageSize = rows;
    this.load(first);
  }

  protected async load(first = this.first()): Promise<void> {
    const info = this.info();
    if (!info) return;
    this.loading.set(true);
    try {
      const page = await inventoryApi.stockCard({
        productId: info.productId,
        batchId: this.batchId,
        dateFrom: this.dateFrom || null,
        dateTo: this.dateTo || null,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.page.set(page);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected batchLabel(b: BatchRow): string {
    return `${b.batchNumber} · ED ${formatDate(b.expiryDate)} · stok ${b.qtyOnHandBase}`;
  }

  protected refLabel(row: StockCardRow): string {
    return row.refNumber ?? `${row.refType} #${row.refId}`;
  }
}
