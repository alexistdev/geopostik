import { Component, OnInit, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';

import type { Supplier } from '../../../bindings/Supplier';
import type { SupplierInput } from '../../../bindings/SupplierInput';
import { purchasingApi } from '../../../core/api/purchasing.api';
import { Notify } from '../../../core/ui/notify';

/** Tambah/ubah supplier (PBF). Dipakai di Master Data › Supplier dan langsung dari form faktur. */
@Component({
  selector: 'app-supplier-dialog',
  imports: [FormsModule, ButtonModule, DialogModule, InputNumberModule, InputTextModule, TagModule, ToggleSwitchModule],
  template: `
    <p-dialog
      [visible]="true"
      (visibleChange)="closed.emit()"
      [modal]="true"
      [draggable]="false"
      [style]="{ width: '560px' }"
      styleClass="master-dialog"
    >
      <ng-template #header>
        <div class="dlg-head">
          <span class="dlg-icon"><i class="pi pi-truck"></i></span>
          <div class="dlg-title">
            <span class="dlg-name">{{ supplier() ? form.name || 'Supplier' : 'Supplier baru' }}</span>
            @if (supplier(); as s) {
              <span class="dlg-meta">
                <span class="mono">{{ s.code }}</span>
                <p-tag [value]="form.isActive ? 'Aktif' : 'Nonaktif'" [severity]="form.isActive ? 'success' : 'secondary'" />
              </span>
            } @else {
              <span class="dlg-meta">Isi data supplier; kode dibuat otomatis.</span>
            }
          </div>
        </div>
      </ng-template>

      <section class="card">
        <header class="card-head">
          <span class="card-icon"><i class="pi pi-building"></i></span>
          <div>
            <h3>Data supplier</h3>
            <p>Pedagang Besar Farmasi (PBF) atau distributor asal faktur pembelian.</p>
          </div>
        </header>
        <div class="grid">
          <div class="field span-2">
            <label for="supName">Nama <span class="req">*</span></label>
            <input pInputText id="supName" [(ngModel)]="form.name" placeholder="Misal: PT Anugrah Pharmindo Lestari" autofocus />
          </div>
          <div class="field">
            <label for="supPhone">Telepon</label>
            <input pInputText id="supPhone" [(ngModel)]="form.phone" />
          </div>
          <div class="field">
            <label for="supNpwp">NPWP</label>
            <input pInputText id="supNpwp" [(ngModel)]="form.npwp" />
          </div>
          <div class="field span-2">
            <label for="supAddr">Alamat</label>
            <input pInputText id="supAddr" [(ngModel)]="form.address" />
          </div>
          <div class="field">
            <label for="supTerm">Tempo pembayaran</label>
            <p-inputnumber inputId="supTerm" [(ngModel)]="form.paymentTermDays" [min]="0" [max]="365" suffix=" hari" [useGrouping]="false" />
            <small>Jatuh tempo faktur kredit diisi otomatis dari tanggal faktur.</small>
          </div>
        </div>
      </section>

      <label class="switch-row">
        <span>
          Aktif
          <small>Muncul sebagai pilihan saat input faktur.</small>
        </span>
        <p-toggleswitch [(ngModel)]="form.isActive" />
      </label>

      <ng-template #footer>
        <p-button label="Batal" severity="secondary" [outlined]="true" (onClick)="closed.emit()" />
        <p-button label="Simpan" icon="pi pi-check" [loading]="saving()" (onClick)="save()" />
      </ng-template>
    </p-dialog>
  `,
  styles: `
    .grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      column-gap: 1rem;
    }
    .span-2 {
      grid-column: span 2;
    }
  `,
})
export class SupplierDialog implements OnInit {
  private readonly notify = inject(Notify);

  /** `null` = supplier baru. */
  readonly supplier = input<Supplier | null>(null);
  /** Nama awal untuk supplier baru (misal teks yang diketik di pencarian). */
  readonly initialName = input('');
  readonly saved = output<Supplier>();
  readonly closed = output<void>();

  protected readonly saving = signal(false);
  protected form: SupplierInput = {
    id: null,
    name: '',
    address: null,
    phone: null,
    npwp: null,
    paymentTermDays: 30,
    isActive: true,
  };

  ngOnInit(): void {
    const s = this.supplier();
    this.form = s
      ? {
          id: s.id,
          name: s.name,
          address: s.address,
          phone: s.phone,
          npwp: s.npwp,
          paymentTermDays: s.paymentTermDays,
          isActive: s.isActive,
        }
      : { ...this.form, name: this.initialName() };
  }

  protected async save(): Promise<void> {
    this.saving.set(true);
    try {
      const supplier = await purchasingApi.supplierSave({ ...this.form, paymentTermDays: this.form.paymentTermDays ?? 0 });
      this.notify.success(`Supplier ${supplier.name} tersimpan`);
      this.saved.emit(supplier);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
