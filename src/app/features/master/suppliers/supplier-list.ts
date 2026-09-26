import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';

import type { Supplier } from '../../../bindings/Supplier';
import { purchasingApi } from '../../../core/api/purchasing.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { DateTimePipe } from '../../../shared/format';
import { MasterRowActions, type MasterRowApi } from '../master-row-actions';
import { MASTER_PAGE_SIZES, MasterTable, PAGE_REPORT } from '../master-table';
import { SupplierDialog } from './supplier-dialog';

/** Master Supplier: asal faktur pembelian. */
@Component({
  selector: 'app-supplier-list',
  imports: [
    FormsModule,
    ButtonModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    TableModule,
    DateTimePipe,
    MasterRowActions,
    SupplierDialog,
  ],
  templateUrl: './supplier-list.html',
  styleUrl: '../people-list.scss',
})
export class SupplierList {
  private readonly notify = inject(Notify);
  protected readonly canEdit = inject(AuthService).can('PURCHASE_RECEIVE');

  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected readonly table = new MasterTable<Supplier>(
    purchasingApi.supplierPage,
    purchasingApi.supplierList,
    (e) => this.notify.error(e),
  );
  protected readonly rowApi: MasterRowApi = {
    setActive: purchasingApi.supplierSetActive,
    remove: purchasingApi.supplierDelete,
  };
  /** `undefined` = dialog tertutup, `null` = supplier baru. */
  protected readonly editing = signal<Supplier | null | undefined>(undefined);

  constructor() {
    this.table.load(0);
  }

  protected open(supplier: Supplier | null): void {
    if (this.canEdit) this.editing.set(supplier);
  }

  protected saved(): void {
    this.editing.set(undefined);
    this.table.load();
  }
}
