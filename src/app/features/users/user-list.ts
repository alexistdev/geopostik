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

import type { BatchResult } from '../../bindings/BatchResult';
import type { Role } from '../../bindings/Role';
import type { UserRow } from '../../bindings/UserRow';
import { userApi } from '../../core/api/user.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { confirmDelete } from '../../shared/confirm';
import { DateTimePipe } from '../../shared/format';
import { ROLES, roleInfo } from '../../shared/labels';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master/master-table';
import { UserDialog } from './user-dialog';

type BatchAction = 'ACTIVATE' | 'DEACTIVATE' | 'DELETE';

const BATCH_ACTIONS: { value: BatchAction; label: string; icon: string }[] = [
  { value: 'ACTIVATE', label: 'Aktifkan', icon: 'pi pi-check-circle' },
  { value: 'DEACTIVATE', label: 'Nonaktifkan', icon: 'pi pi-ban' },
  { value: 'DELETE', label: 'Hapus', icon: 'pi pi-trash' },
];

/** Menu Pengguna (hak USER_MANAGE). Tabel mengikuti tabel Obat. */
@Component({
  selector: 'app-user-list',
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
    DateTimePipe,
    UserDialog,
  ],
  templateUrl: './user-list.html',
  styleUrl: '../master/products/product-list.scss',
})
export class UserList {
  private readonly notify = inject(Notify);
  private readonly confirm = inject(ConfirmationService);
  private readonly auth = inject(AuthService);

  protected readonly roles = ROLES;
  protected readonly roleInfo = roleInfo;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected pageSize = MASTER_PAGE_SIZES[0];

  protected readonly rows = signal<UserRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);

  protected q = '';
  protected role: Role | null = null;
  protected includeInactive = false;
  protected readonly first = signal(0);
  private searchTimer: ReturnType<typeof setTimeout> | undefined;
  /** Baris yang dicentang (tetap tersimpan saat pindah halaman) untuk aksi massal. */
  protected selected: UserRow[] = [];
  protected readonly batchActions = BATCH_ACTIONS;
  /** Pilihan dropdown aksi massal; dikosongkan lagi setelah dijalankan atau dibatalkan. */
  protected batchAction: BatchAction | null = null;
  protected readonly batchRunning = signal(false);

  /** `undefined` = dialog tertutup, `null` = pengguna baru. */
  protected readonly editing = signal<UserRow | null | undefined>(undefined);

  /** Akun yang sedang login: tidak bisa dinonaktifkan atau dihapus dari sini. */
  protected isSelf(row: UserRow): boolean {
    return row.id === this.auth.user()?.id;
  }

  protected async load(first = this.first()): Promise<void> {
    this.first.set(first);
    this.loading.set(true);
    try {
      const result = await userApi.list({
        q: this.q.trim() || null,
        role: this.role,
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

  protected async toggleActive(row: UserRow): Promise<void> {
    try {
      await userApi.setActive(row.id, !row.isActive);
      this.notify.success(`Pengguna ${row.fullName} ${row.isActive ? 'dinonaktifkan' : 'diaktifkan'}`);
      await this.load();
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected confirmDelete(row: UserRow): void {
    confirmDelete(this.confirm, {
      header: 'Hapus pengguna?',
      message: 'Pengguna ini akan disembunyikan dari daftar dan tidak bisa login lagi.',
      item: { name: row.fullName, code: row.username },
      note: 'Riwayat transaksi dan log-nya tetap tersimpan. Username tidak bisa dipakai lagi.',
      accept: () => this.delete(row),
    });
  }

  private async delete(row: UserRow): Promise<void> {
    try {
      await userApi.delete(row.id);
      this.notify.success(`Pengguna ${row.fullName} dihapus`);
      this.selected = this.selected.filter((s) => s.id !== row.id);
      // Halaman jadi kosong setelah hapus baris terakhirnya → mundur satu halaman.
      const first = this.first();
      await this.load(this.rows().length === 1 && first > 0 ? Math.max(0, first - this.pageSize) : first);
    } catch (e) {
      this.notify.error(e);
    }
  }

  /** Konfirmasi lalu jalankan aksi massal pada pengguna yang dicentang. */
  protected runBatch(action: BatchAction | null): void {
    const rows = this.selected;
    if (!action || !rows.length) return;
    const n = rows.length;
    const label = BATCH_ACTIONS.find((a) => a.value === action)!.label;
    const accept = () => this.executeBatch(action, rows);
    const reject = () => (this.batchAction = null);
    if (action === 'DELETE') {
      confirmDelete(this.confirm, {
        header: `Hapus ${n} pengguna?`,
        message: 'Pengguna yang dicentang akan disembunyikan dari daftar dan tidak bisa login lagi.',
        item: { name: `${n} pengguna dicentang` },
        note: 'Akun Anda sendiri dan pemilik aktif terakhir dilewati. Riwayatnya tetap tersimpan.',
        acceptLabel: `Hapus ${n} pengguna`,
        accept,
        reject,
      });
      return;
    }
    this.confirm.confirm({
      header: `${label} ${n} pengguna?`,
      message: `${label} ${n} pengguna yang dicentang?`,
      icon: action === 'ACTIVATE' ? 'pi pi-check-circle' : 'pi pi-ban',
      acceptLabel: label,
      rejectLabel: 'Batal',
      rejectButtonProps: { severity: 'secondary', outlined: true },
      accept,
      reject,
    });
  }

  private async executeBatch(action: BatchAction, rows: UserRow[]): Promise<void> {
    const ids = rows.map((r) => r.id);
    this.batchRunning.set(true);
    try {
      const result: BatchResult =
        action === 'DELETE'
          ? await userApi.deleteMany(ids)
          : await userApi.setActiveMany(ids, action === 'ACTIVATE');
      const verb = { ACTIVATE: 'diaktifkan', DEACTIVATE: 'dinonaktifkan', DELETE: 'dihapus' }[action];
      if (result.done) {
        this.notify.success(`${result.done} pengguna ${verb}`);
      } else if (!result.skipped.length) {
        this.notify.success(`Tidak ada perubahan: semua pengguna sudah ${verb}`);
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
