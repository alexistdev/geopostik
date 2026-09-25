import { Component, computed, inject, signal } from '@angular/core';
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
import { bpToPercent, percentToBp } from '../../../shared/format';
import { LabelPrint } from '../../../shared/label-print';
import { MasterRowActions } from '../master-row-actions';

interface CategoryForm {
  id: number | null;
  code: string;
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

  protected readonly categories = signal<Category[]>([]);
  protected readonly filter = signal('');
  /** Cari nama, atau kode (termasuk hasil scan label barcode). */
  protected readonly visibleCategories = computed(() => {
    const q = this.filter().trim().toLowerCase();
    return q
      ? this.categories().filter((c) => c.name.toLowerCase().includes(q) || c.code.toLowerCase().includes(q))
      : this.categories();
  });
  protected selected: Category[] = [];
  protected readonly saving = signal(false);
  protected readonly bpToPercent = bpToPercent;
  protected form: CategoryForm | null = null;

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    try {
      this.selected = [];
      this.categories.set(await masterApi.categoryList());
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected open(c?: Category): void {
    if (!this.canEdit) return;
    this.form = c
      ? { id: c.id, code: c.code, name: c.name, marginPercent: bpToPercent(c.marginBp), isActive: c.isActive }
      : { id: null, code: '', name: '', marginPercent: null, isActive: true };
  }

  /** Cetak label barcode untuk baris yang dicentang, atau semua yang tampil bila tidak ada yang dicentang. */
  protected printLabels(): void {
    this.labels.print('Label Kategori', this.selected.length ? this.selected : this.visibleCategories());
  }

  protected async save(): Promise<void> {
    const f = this.form;
    if (!f) return;
    this.saving.set(true);
    try {
      await masterApi.categorySave({
        id: f.id,
        code: f.code.trim() || null,
        name: f.name,
        marginBp: percentToBp(f.marginPercent),
        isActive: f.isActive,
      });
      this.form = null;
      this.notify.success('Kategori tersimpan');
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
