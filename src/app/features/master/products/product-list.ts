import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { Category } from '../../../bindings/Category';
import type { DrugClass } from '../../../bindings/DrugClass';
import type { ProductListRow } from '../../../bindings/ProductListRow';
import { masterApi } from '../../../core/api/master.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { RupiahPipe } from '../../../shared/format';
import { DRUG_CLASSES, drugClassInfo } from '../../../shared/labels';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master-table';
import { ProductDialog } from './product-dialog';

@Component({
  selector: 'app-product-list',
  imports: [
    FormsModule,
    ButtonModule,
    CheckboxModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    SelectModule,
    TableModule,
    TagModule,
    RupiahPipe,
    ProductDialog,
  ],
  templateUrl: './product-list.html',
  styleUrl: './product-list.scss',
})
export class ProductList {
  private readonly notify = inject(Notify);
  protected readonly auth = inject(AuthService);

  protected readonly drugClasses = DRUG_CLASSES;
  protected readonly drugClassInfo = drugClassInfo;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected pageSize = MASTER_PAGE_SIZES[0];

  protected readonly rows = signal<ProductListRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);
  protected readonly categories = signal<Category[]>([]);

  protected q = '';
  protected categoryId: number | null = null;
  protected drugClass: DrugClass | null = null;
  protected includeInactive = false;
  protected readonly first = signal(0);
  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  /** `undefined` = dialog tertutup, `null` = obat baru. */
  protected readonly editing = signal<number | null | undefined>(undefined);

  constructor() {
    masterApi.categoryList().then(
      (c) => this.categories.set(c.filter((x) => x.isActive)),
      (e) => this.notify.error(e),
    );
  }

  protected async load(first = this.first()): Promise<void> {
    this.first.set(first);
    this.loading.set(true);
    try {
      const result = await masterApi.productList({
        q: this.q.trim() || null,
        categoryId: this.categoryId,
        drugClass: this.drugClass,
        includeInactive: this.includeInactive,
        offset: first,
        limit: this.pageSize,
      });
      this.rows.set(result.rows);
      this.total.set(result.total);
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

  /** Pencarian menunggu ketikan berhenti sebentar agar tidak memanggil backend tiap huruf. */
  protected searchChanged(): void {
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.load(0), 250);
  }

  protected isLowStock(row: ProductListRow): boolean {
    return row.isActive && row.minStockBase > 0 && row.stockBase < row.minStockBase;
  }

  protected closeDialog(changed: boolean): void {
    this.editing.set(undefined);
    if (changed) {
      this.load();
    }
  }
}
