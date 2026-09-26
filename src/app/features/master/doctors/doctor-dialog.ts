import { Component, OnInit, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';

import type { Doctor } from '../../../bindings/Doctor';
import type { DoctorInput } from '../../../bindings/DoctorInput';
import { prescriptionApi } from '../../../core/api/prescription.api';
import { Notify } from '../../../core/ui/notify';

/** Tambah/ubah dokter. Dipakai di Master Data › Dokter dan langsung dari form resep. */
@Component({
  selector: 'app-doctor-dialog',
  imports: [FormsModule, ButtonModule, DialogModule, InputTextModule, TagModule, ToggleSwitchModule],
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
          <span class="dlg-icon"><i class="pi pi-user"></i></span>
          <div class="dlg-title">
            <span class="dlg-name">{{ doctor() ? form.name || 'Dokter' : 'Dokter baru' }}</span>
            @if (doctor(); as d) {
              <span class="dlg-meta">
                <span class="mono">{{ d.code }}</span>
                <p-tag [value]="form.isActive ? 'Aktif' : 'Nonaktif'" [severity]="form.isActive ? 'success' : 'secondary'" />
              </span>
            } @else {
              <span class="dlg-meta">Isi data dokter; kode dibuat otomatis.</span>
            }
          </div>
        </div>
      </ng-template>

      <section class="card">
        <header class="card-head">
          <span class="card-icon"><i class="pi pi-id-card"></i></span>
          <div>
            <h3>Data dokter</h3>
            <p>Nama dan nomor SIP tercetak di copy resep dan laporan narkotika/psikotropika.</p>
          </div>
        </header>
        <div class="grid">
          <div class="field span-2">
            <label for="docName">Nama <span class="req">*</span></label>
            <input pInputText id="docName" [(ngModel)]="form.name" placeholder="Misal: dr. Andi Wijaya, Sp.A" autofocus />
          </div>
          <div class="field">
            <label for="docSip">No. SIP</label>
            <input pInputText id="docSip" [(ngModel)]="form.sipNumber" placeholder="Surat Izin Praktik" />
          </div>
          <div class="field">
            <label for="docSpec">Spesialis</label>
            <input pInputText id="docSpec" [(ngModel)]="form.specialty" placeholder="Misal: Anak, Umum" />
          </div>
          <div class="field">
            <label for="docPhone">Telepon</label>
            <input pInputText id="docPhone" [(ngModel)]="form.phone" />
          </div>
          <div class="field">
            <label for="docAddr">Alamat praktik</label>
            <input pInputText id="docAddr" [(ngModel)]="form.address" />
          </div>
        </div>
      </section>

      <label class="switch-row">
        <span>
          Aktif
          <small>Muncul sebagai pilihan saat input resep.</small>
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
export class DoctorDialog implements OnInit {
  private readonly notify = inject(Notify);

  /** `null` = dokter baru. */
  readonly doctor = input<Doctor | null>(null);
  /** Nama awal untuk dokter baru (misal teks yang diketik di pencarian). */
  readonly initialName = input('');
  readonly saved = output<Doctor>();
  readonly closed = output<void>();

  protected readonly saving = signal(false);
  protected form: DoctorInput = {
    id: null,
    name: '',
    sipNumber: null,
    specialty: null,
    address: null,
    phone: null,
    isActive: true,
  };

  ngOnInit(): void {
    const d = this.doctor();
    this.form = d
      ? {
          id: d.id,
          name: d.name,
          sipNumber: d.sipNumber,
          specialty: d.specialty,
          address: d.address,
          phone: d.phone,
          isActive: d.isActive,
        }
      : { ...this.form, name: this.initialName() };
  }

  protected async save(): Promise<void> {
    this.saving.set(true);
    try {
      const doctor = await prescriptionApi.doctorSave(this.form);
      this.notify.success(`Dokter ${doctor.name} tersimpan`);
      this.saved.emit(doctor);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
