import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { Category } from '../../../bindings/Category';
import { masterApi } from '../../../core/api/master.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { Barcode } from '../../../shared/barcode';
import { bpToPercent, DateTimePipe, percentToBp } from '../../../shared/format';
import { LabelPrint } from '../../../shared/label-print';
import { MasterRowActions } from '../master-row-actions';
import { MASTER_PAGE_SIZES, MasterTable, PAGE_REPORT } from '../master-table';

interface CategoryForm {
  id: number | null;
  /** Hanya untuk ditampilkan; kode dibuat dan dijaga oleh sistem. */
  code: string | null;
  name: string;
  marginPercent: number | null;
  isActive: boolean;
}

@Component({
  selector: 'app-category-list',
  imports: [
    FormsModule,
    ButtonModule,
    CheckboxModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputNumberModule,
    InputTextModule,
    TableModule,
    TagModule,
    Barcode,
    DateTimePipe,
    MasterRowActions,
  ],
  templateUrl: './category-list.html',
  styles: `
    .toolbar {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      margin-bottom: 0.75rem;
    }
    .spacer {
      flex: 1;
    }
    .code {
      font-family: ui-monospace, Consolas, monospace;
    }
    .preview {
      text-align: center;
      margin: 0.5rem 0 1rem;
    }
    .muted {
      color: var(--p-text-muted-color);
    }
    .check {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    p-inputnumber {
      width: 100%;
    }
  `,
})
export class CategoryList {
  private readonly notify = inject(Notify);
  private readonly labels = inject(LabelPrint);
  protected readonly auth = inject(AuthService);
  protected readonly canEdit = this.auth.can('PRODUCT_MANAGE');
  protected readonly canEditMargin = this.auth.can('PRICE_MANAGE');
  protected readonly showMargin = this.auth.can('PRICE_MANAGE') || this.auth.can('VIEW_COST');

  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  /** Margin global untuk kategori tanpa margin sendiri, dari halaman terakhir yang dimuat. */
  protected readonly defaultMarginBp = signal<number | null>(null);
  protected readonly table = new MasterTable<Category>(
    async (query) => {
      const page = await masterApi.categoryPage(query);
      this.defaultMarginBp.set(page.defaultMarginBp);
      return page;
    },
    masterApi.categoryList,
    (e) => this.notify.error(e),
  );
  protected readonly saving = signal(false);
  protected readonly bpToPercent = bpToPercent;
  protected form: CategoryForm | null = null;

  constructor() {
    this.table.load(0);
  }

  protected open(c?: Category): void {
    if (!this.canEdit) return;
    this.form = c
      ? { id: c.id, code: c.code, name: c.name, marginPercent: bpToPercent(c.marginBp), isActive: c.isActive }
      : { id: null, code: null, name: '', marginPercent: null, isActive: true };
  }

  /** Cetak label barcode untuk baris yang dicentang, atau semua yang cocok dengan pencarian. */
  protected async printLabels(): Promise<void> {
    try {
      this.labels.print('Label Kategori', await this.table.itemsToPrint());
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected async save(): Promise<void> {
    const f = this.form;
    if (!f) return;
    this.saving.set(true);
    try {
      await masterApi.categorySave({
        id: f.id,
        name: f.name,
        marginBp: percentToBp(f.marginPercent),
        isActive: f.isActive,
      });
      this.form = null;
      this.notify.success('Kategori tersimpan');
      await this.table.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
