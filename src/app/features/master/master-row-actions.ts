import { Component, inject, input, output } from '@angular/core';
import { ConfirmationService } from 'primeng/api';
import { ButtonModule } from 'primeng/button';
import { TooltipModule } from 'primeng/tooltip';

import type { MasterKind } from '../../bindings/MasterKind';
import { masterApi } from '../../core/api/master.api';
import { Notify } from '../../core/ui/notify';
import { confirmDelete } from '../../shared/confirm';

/** Aksi aktif/nonaktif & hapus untuk jenis data di luar `MasterKind` (misal dokter, pasien). */
export interface MasterRowApi {
  setActive: (id: number, active: boolean) => Promise<void>;
  remove: (id: number) => Promise<void>;
}

export interface MasterRow {
  id: number;
  code: string;
  name: string;
  isActive: boolean;
}

/** Tombol Edit, Nonaktifkan/Aktifkan, dan Hapus untuk satu baris Master Data. */
@Component({
  selector: 'app-master-row-actions',
  imports: [ButtonModule, TooltipModule],
  template: `
    <p-button icon="pi pi-pencil" size="small" [rounded]="true" [text]="true" pTooltip="Edit" tooltipPosition="top" ariaLabel="Edit" (onClick)="edit.emit()" />
    <p-button
      [icon]="row().isActive ? 'pi pi-ban' : 'pi pi-check-circle'"
      size="small"
      [severity]="row().isActive ? 'secondary' : 'success'"
      [rounded]="true"
      [text]="true"
      [pTooltip]="row().isActive ? 'Nonaktifkan' : 'Aktifkan'"
      tooltipPosition="top"
      [ariaLabel]="row().isActive ? 'Nonaktifkan' : 'Aktifkan'"
      (onClick)="toggleActive()"
    />
    <p-button icon="pi pi-trash" size="small" severity="danger" [rounded]="true" [text]="true" pTooltip="Hapus" tooltipPosition="top" ariaLabel="Hapus" (onClick)="confirmDelete()" />
  `,
  styles: `
    :host {
      display: flex;
      gap: 0.125rem;
      justify-content: flex-end;
      opacity: 0.55;
      transition: opacity 0.12s;
    }
    :host-context(tr:hover),
    :host(:focus-within) {
      opacity: 1;
    }
  `,
})
export class MasterRowActions {
  private readonly notify = inject(Notify);
  private readonly confirm = inject(ConfirmationService);

  /** Jenis data Master Data; diabaikan bila `api` diisi. */
  readonly kind = input<MasterKind>();
  readonly api = input<MasterRowApi>();
  /** Nama jenis data untuk pesan, misal "Rak". */
  readonly label = input.required<string>();
  readonly row = input.required<MasterRow>();
  readonly edit = output<void>();
  /** Data berubah (status atau terhapus); induk memuat ulang daftar. */
  readonly changed = output<void>();
  /** Pesan konfirmasi hapus; default untuk data pilihan form obat. */
  readonly deleteMessage = input<string>();

  private rowApi(): MasterRowApi {
    const api = this.api();
    if (api) return api;
    const kind = this.kind()!;
    return {
      setActive: (id, active) => masterApi.masterSetActive(kind, id, active),
      remove: (id) => masterApi.masterDelete(kind, id),
    };
  }

  protected async toggleActive(): Promise<void> {
    const r = this.row();
    try {
      await this.rowApi().setActive(r.id, !r.isActive);
      this.notify.success(`${this.label()} ${r.name} ${r.isActive ? 'dinonaktifkan' : 'diaktifkan'}`);
      this.changed.emit();
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected confirmDelete(): void {
    const r = this.row();
    confirmDelete(this.confirm, {
      header: `Hapus ${this.label().toLowerCase()}?`,
      message:
        this.deleteMessage() ??
        `${this.label()} ini akan disembunyikan dari daftar dan pilihan saat tambah/ubah obat.`,
      item: { name: r.name, code: r.code },
      note: 'Riwayatnya tetap tersimpan.',
      accept: () => this.delete(),
    });
  }

  private async delete(): Promise<void> {
    const r = this.row();
    try {
      await this.rowApi().remove(r.id);
      this.notify.success(`${this.label()} ${r.name} dihapus`);
      this.changed.emit();
    } catch (e) {
      this.notify.error(e);
    }
  }
}
