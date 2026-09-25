import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ConfirmationService } from 'primeng/api';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { TooltipModule } from 'primeng/tooltip';

import type { BatchResult } from '../../../bindings/BatchResult';
import type { Category } from '../../../bindings/Category';
import type { DrugClass } from '../../../bindings/DrugClass';
import type { ProductListRow } from '../../../bindings/ProductListRow';
import { masterApi } from '../../../core/api/master.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { RupiahPipe } from '../../../shared/format';
import { LabelPrint } from '../../../shared/label-print';
import { DRUG_CLASSES, drugClassInfo } from '../../../shared/labels';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master-table';
import { ProductDialog } from './product-dialog';

type BatchAction = 'ACTIVATE' | 'DEACTIVATE' | 'DELETE';

const BATCH_ACTIONS: { value: BatchAction; label: string; icon: string }[] = [
  { value: 'ACTIVATE', label: 'Aktifkan', icon: 'pi pi-check-circle' },
  { value: 'DEACTIVATE', label: 'Nonaktifkan', icon: 'pi pi-ban' },
  { value: 'DELETE', label: 'Hapus', icon: 'pi pi-trash' },
];

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
    TooltipModule,
    RupiahPipe,
    ProductDialog,
  ],
  templateUrl: './product-list.html',
  styleUrl: './product-list.scss',
})
export class ProductList {
  private readonly notify = inject(Notify);
  private readonly confirm = inject(ConfirmationService);
  private readonly labels = inject(LabelPrint);
  protected readonly canEdit = inject(AuthService).can('PRODUCT_MANAGE');

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
  /** Baris yang dicentang (tetap tersimpan saat pindah halaman) untuk cetak label. */
  protected selected: ProductListRow[] = [];
  protected readonly batchActions = BATCH_ACTIONS;
  /** Pilihan dropdown aksi massal; dikosongkan lagi setelah dijalankan atau dibatalkan. */
  protected batchAction: BatchAction | null = null;
  protected readonly batchRunning = signal(false);

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

  /** Cetak label barcode untuk obat yang dicentang, atau semua obat yang cocok dengan filter. */
  protected async printLabels(): Promise<void> {
    try {
      this.labels.print('Label Obat', this.selected.length ? this.selected : await this.fetchAllMatching());
    } catch (e) {
      this.notify.error(e);
    }
  }

  private async fetchAllMatching(): Promise<ProductListRow[]> {
    const all: ProductListRow[] = [];
    const limit = 500;
    for (let offset = 0; ; offset += limit) {
      const page = await masterApi.productList({
        q: this.q.trim() || null,
        categoryId: this.categoryId,
        drugClass: this.drugClass,
        includeInactive: this.includeInactive,
        offset,
        limit,
      });
      all.push(...page.rows);
      if (all.length >= page.total || !page.rows.length) {
        return all;
      }
    }
  }

  protected async toggleActive(row: ProductListRow): Promise<void> {
    try {
      await masterApi.productSetActive(row.id, !row.isActive);
      this.notify.success(`Obat ${row.name} ${row.isActive ? 'dinonaktifkan' : 'diaktifkan'}`);
      await this.load();
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected confirmDelete(row: ProductListRow): void {
    this.confirm.confirm({
      header: 'Hapus obat',
      message: `Hapus obat "${row.name}" (${row.code})? Obat akan disembunyikan dari daftar dan pencarian, tetapi riwayatnya tetap tersimpan.`,
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Hapus',
      rejectLabel: 'Batal',
      acceptButtonProps: { severity: 'danger' },
      rejectButtonProps: { severity: 'secondary', text: true },
      accept: () => this.delete(row),
    });
  }

  private async delete(row: ProductListRow): Promise<void> {
    try {
      await masterApi.productDelete(row.id);
      this.notify.success(`Obat ${row.name} dihapus`);
      this.selected = this.selected.filter((s) => s.id !== row.id);
      // Halaman jadi kosong setelah hapus baris terakhirnya → mundur satu halaman.
      const first = this.first();
      await this.load(this.rows().length === 1 && first > 0 ? Math.max(0, first - this.pageSize) : first);
    } catch (e) {
      this.notify.error(e);
    }
  }

  /** Konfirmasi lalu jalankan aksi massal pada obat yang dicentang. */
  protected runBatch(action: BatchAction | null): void {
    const rows = this.selected;
    if (!action || !rows.length) return;
    const n = rows.length;
    const label = BATCH_ACTIONS.find((a) => a.value === action)!.label;
    this.confirm.confirm({
      header: `${label} ${n} obat`,
      message:
        action === 'DELETE'
          ? `Hapus ${n} obat yang dicentang? Obat akan disembunyikan dari daftar dan pencarian, tetapi riwayatnya tetap tersimpan. Obat yang masih punya stok dilewati.`
          : `${label} ${n} obat yang dicentang?`,
      icon: action === 'DELETE' ? 'pi pi-exclamation-triangle' : 'pi pi-question-circle',
      acceptLabel: label,
      rejectLabel: 'Batal',
      acceptButtonProps: { severity: action === 'DELETE' ? 'danger' : undefined },
      rejectButtonProps: { severity: 'secondary', text: true },
      accept: () => this.executeBatch(action, rows, label),
      reject: () => (this.batchAction = null),
    });
  }

  private async executeBatch(action: BatchAction, rows: ProductListRow[], label: string): Promise<void> {
    const ids = rows.map((r) => r.id);
    this.batchRunning.set(true);
    try {
      const result: BatchResult =
        action === 'DELETE'
          ? await masterApi.productDeleteMany(ids)
          : await masterApi.productSetActiveMany(ids, action === 'ACTIVATE');
      const verb = { ACTIVATE: 'diaktifkan', DEACTIVATE: 'dinonaktifkan', DELETE: 'dihapus' }[action];
      if (result.done) {
        this.notify.success(`${result.done} obat ${verb}`);
      } else if (!result.skipped.length) {
        this.notify.success(`Tidak ada perubahan: semua obat sudah ${verb}`);
      }
      this.notify.warnings(result.skipped);
      this.selected = [];
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.batchAction = null;
      this.batchRunning.set(false);
    }
  }

  protected closeDialog(changed: boolean): void {
    this.editing.set(undefined);
    if (changed) {
      this.load();
    }
  }
}
