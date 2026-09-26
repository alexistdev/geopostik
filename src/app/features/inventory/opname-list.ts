import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { ConfirmationService } from 'primeng/api';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { OpnameRow } from '../../bindings/OpnameRow';
import type { OpnameStatus } from '../../bindings/OpnameStatus';
import type { OpnameType } from '../../bindings/OpnameType';
import { inventoryApi } from '../../core/api/inventory.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe } from '../../shared/format';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master/master-table';
import { OPNAME_STATUSES, OPNAME_TYPES, opnameStatusInfo } from './stock-format';

/** Tab Stok Opname: daftar dokumen opname (stok awal & berkala). */
@Component({
  selector: 'app-opname-list',
  imports: [
    FormsModule,
    ButtonModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    SelectModule,
    TableModule,
    TagModule,
    DateTimePipe,
  ],
  templateUrl: './opname-list.html',
  styleUrl: './opname-list.scss',
})
export class OpnameList {
  private readonly notify = inject(Notify);
  private readonly router = inject(Router);
  private readonly confirm = inject(ConfirmationService);
  protected readonly canApprove = inject(AuthService).can('STOCK_COUNT_APPROVE');

  protected readonly statuses = OPNAME_STATUSES;
  protected readonly types = OPNAME_TYPES;
  protected readonly typeKeys = Object.keys(OPNAME_TYPES) as OpnameType[];
  protected readonly statusInfo = opnameStatusInfo;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected pageSize = MASTER_PAGE_SIZES[0];

  protected readonly rows = signal<OpnameRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);
  protected readonly openingLocked = signal(false);

  protected q = '';
  protected status: OpnameStatus | null = null;
  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  protected creating: { type: OpnameType; scopeNote: string } | null = null;
  protected readonly saving = signal(false);

  protected async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await inventoryApi.opnamePage({
        q: this.q.trim() || null,
        status: this.status,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
      this.openingLocked.set(page.openingLocked);
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

  protected newOpname(): void {
    this.creating = { type: this.openingLocked() ? 'PERIODIC' : 'OPENING', scopeNote: '' };
  }

  protected async create(): Promise<void> {
    const c = this.creating;
    if (!c) return;
    this.saving.set(true);
    try {
      const o = await inventoryApi.opnameCreate({ opnameType: c.type, scopeNote: c.scopeNote.trim() || null });
      this.creating = null;
      this.notify.success(`Opname ${o.header.number} dibuat`);
      this.open(o.header);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  protected open(row: Pick<OpnameRow, 'id'>): void {
    this.router.navigate(['/stok/opname', row.id]);
  }

  protected confirmLockOpening(): void {
    this.confirm.confirm({
      header: 'Kunci stok awal?',
      message:
        'Setelah dikunci, opname stok awal tidak bisa dibuat lagi. Stok hanya berubah lewat transaksi biasa dan opname berkala.',
      icon: 'pi pi-lock',
      acceptLabel: 'Kunci stok awal',
      acceptIcon: 'pi pi-lock',
      rejectLabel: 'Batal',
      rejectButtonProps: { severity: 'secondary', outlined: true },
      accept: async () => {
        try {
          await inventoryApi.openingLock();
          this.notify.success('Stok awal dikunci');
          await this.load();
        } catch (e) {
          this.notify.error(e);
        }
      },
    });
  }
}
