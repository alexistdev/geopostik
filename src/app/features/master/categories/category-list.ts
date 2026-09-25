import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { Category } from '../../../bindings/Category';
import { masterApi } from '../../../core/api/master.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { bpToPercent, percentToBp } from '../../../shared/format';

interface CategoryForm {
  id: number | null;
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
    InputNumberModule,
    InputTextModule,
    TableModule,
    TagModule,
  ],
  templateUrl: './category-list.html',
  styles: `
    .toolbar {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 0.75rem;
    }
    .clickable {
      cursor: pointer;
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
  protected readonly auth = inject(AuthService);
  protected readonly canEdit = this.auth.can('PRODUCT_MANAGE');
  protected readonly canEditMargin = this.auth.can('PRICE_MANAGE');
  protected readonly showMargin = this.auth.can('PRICE_MANAGE') || this.auth.can('VIEW_COST');

  protected readonly categories = signal<Category[]>([]);
  protected readonly saving = signal(false);
  protected readonly bpToPercent = bpToPercent;
  protected form: CategoryForm | null = null;

  constructor() {
    this.load();
  }

  private async load(): Promise<void> {
    try {
      this.categories.set(await masterApi.categoryList());
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected open(c?: Category): void {
    if (!this.canEdit) return;
    this.form = c
      ? { id: c.id, name: c.name, marginPercent: bpToPercent(c.marginBp), isActive: c.isActive }
      : { id: null, name: '', marginPercent: null, isActive: true };
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
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
