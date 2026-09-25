import { Component, inject, input, output } from '@angular/core';
import { ConfirmationService } from 'primeng/api';
import { ButtonModule } from 'primeng/button';
import { TooltipModule } from 'primeng/tooltip';

import type { MasterKind } from '../../bindings/MasterKind';
import { masterApi } from '../../core/api/master.api';
import { Notify } from '../../core/ui/notify';

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
    <p-button icon="pi pi-pencil" label="Edit" size="small" [text]="true" (onClick)="edit.emit()" />
    <p-button
      [icon]="row().isActive ? 'pi pi-ban' : 'pi pi-check-circle'"
      [label]="row().isActive ? 'Nonaktifkan' : 'Aktifkan'"
      size="small"
      severity="secondary"
      [text]="true"
      (onClick)="toggleActive()"
    />
    <p-button icon="pi pi-trash" label="Hapus" size="small" severity="danger" [text]="true" (onClick)="confirmDelete()" />
  `,
  styles: ':host { display: flex; gap: 0.25rem; justify-content: flex-end; }',
})
export class MasterRowActions {
  private readonly notify = inject(Notify);
  private readonly confirm = inject(ConfirmationService);

  readonly kind = input.required<MasterKind>();
  /** Nama jenis data untuk pesan, misal "Rak". */
  readonly label = input.required<string>();
  readonly row = input.required<MasterRow>();
  readonly edit = output<void>();
  /** Data berubah (status atau terhapus); induk memuat ulang daftar. */
  readonly changed = output<void>();

  protected async toggleActive(): Promise<void> {
    const r = this.row();
    try {
      await masterApi.masterSetActive(this.kind(), r.id, !r.isActive);
      this.notify.success(`${this.label()} ${r.name} ${r.isActive ? 'dinonaktifkan' : 'diaktifkan'}`);
      this.changed.emit();
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected confirmDelete(): void {
    const r = this.row();
    this.confirm.confirm({
      header: `Hapus ${this.label().toLowerCase()}`,
      message: `Hapus ${this.label().toLowerCase()} "${r.name}" (${r.code})? Data akan disembunyikan dari daftar dan pilihan, tetapi riwayatnya tetap tersimpan.`,
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Hapus',
      rejectLabel: 'Batal',
      acceptButtonProps: { severity: 'danger' },
      rejectButtonProps: { severity: 'secondary', text: true },
      accept: () => this.delete(),
    });
  }

  private async delete(): Promise<void> {
    const r = this.row();
    try {
      await masterApi.masterDelete(this.kind(), r.id);
      this.notify.success(`${this.label()} ${r.name} dihapus`);
      this.changed.emit();
    } catch (e) {
      this.notify.error(e);
    }
  }
}
