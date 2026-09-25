import { Component, computed, effect, inject, input, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';

import type { NamedItem } from '../../../bindings/NamedItem';
import type { NamedItemInput } from '../../../bindings/NamedItemInput';
import { masterApi } from '../../../core/api/master.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { Barcode } from '../../../shared/barcode';
import { LabelPrint } from '../../../shared/label-print';
import type { MasterKind } from '../../../bindings/MasterKind';
import { MasterRowActions } from '../master-row-actions';

export type NamedKind = 'rack' | 'manufacturer';

const KINDS: Record<
  NamedKind,
  {
    kind: MasterKind;
    label: string;
    hint: string;
    placeholder: string;
    list: () => Promise<NamedItem[]>;
    save: (input: NamedItemInput) => Promise<NamedItem>;
  }
> = {
  rack: {
    kind: 'RACK',
    label: 'Rak',
    hint: 'Lokasi penyimpanan obat, dipilih saat tambah/ubah obat dan dicetak di lembar stok opname.',
    placeholder: 'Misal: A1, Etalase Depan, Kulkas',
    list: masterApi.rackList,
    save: masterApi.rackSave,
  },
  manufacturer: {
    kind: 'MANUFACTURER',
    label: 'Pabrik',
    hint: 'Pabrik pembuat obat, dipilih saat tambah/ubah obat.',
    placeholder: 'Misal: Kimia Farma',
    list: masterApi.manufacturerList,
    save: masterApi.manufacturerSave,
  },
};

interface Form {
  id: number | null;
  code: string;
  name: string;
  isActive: boolean;
}

/** Daftar master yang hanya berisi nama: Rak dan Pabrik. Jenisnya dari data rute (`kind`). */
@Component({
  selector: 'app-named-list',
  imports: [
    FormsModule,
    ButtonModule,
    CheckboxModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    TableModule,
    TagModule,
    Barcode,
    MasterRowActions,
  ],
  templateUrl: './named-list.html',
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
    .muted {
      color: var(--p-text-muted-color);
    }
    .check {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .code {
      font-family: ui-monospace, Consolas, monospace;
    }
    .preview {
      text-align: center;
      margin-top: 0.5rem;
    }
  `,
})
export class NamedList {
  private readonly notify = inject(Notify);
  private readonly labels = inject(LabelPrint);
  protected readonly canEdit = inject(AuthService).can('PRODUCT_MANAGE');

  readonly kind = input.required<NamedKind>();
  protected readonly config = computed(() => KINDS[this.kind()]);

  protected readonly items = signal<NamedItem[]>([]);
  protected readonly filter = signal('');
  /** Cari nama, atau kode (termasuk hasil scan label barcode). */
  protected readonly visibleItems = computed(() => {
    const q = this.filter().trim().toLowerCase();
    return q
      ? this.items().filter((i) => i.name.toLowerCase().includes(q) || i.code.toLowerCase().includes(q))
      : this.items();
  });
  protected selected: NamedItem[] = [];
  protected readonly saving = signal(false);
  protected form: Form | null = null;

  constructor() {
    effect(() => {
      this.kind();
      this.load();
    });
  }

  protected async load(): Promise<void> {
    try {
      this.selected = [];
      this.items.set(await this.config().list());
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected open(item?: NamedItem): void {
    if (!this.canEdit) return;
    this.form = item
      ? { id: item.id, code: item.code, name: item.name, isActive: item.isActive }
      : { id: null, code: '', name: '', isActive: true };
  }

  /** Cetak label barcode untuk baris yang dicentang, atau semua yang tampil bila tidak ada yang dicentang. */
  protected printLabels(): void {
    const items = this.selected.length ? this.selected : this.visibleItems();
    this.labels.print(`Label ${this.config().label}`, items);
  }

  protected async save(): Promise<void> {
    const f = this.form;
    if (!f) return;
    this.saving.set(true);
    try {
      await this.config().save({ id: f.id, code: f.code.trim() || null, name: f.name, isActive: f.isActive });
      this.form = null;
      this.notify.success(`${this.config().label} tersimpan`);
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
