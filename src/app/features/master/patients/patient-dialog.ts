import { Component, OnInit, computed, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { SelectButtonModule } from 'primeng/selectbutton';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';

import type { Patient } from '../../../bindings/Patient';
import type { PatientInput } from '../../../bindings/PatientInput';
import { prescriptionApi } from '../../../core/api/prescription.api';
import { Notify } from '../../../core/ui/notify';
import { GENDERS, ageFromBirthDate, todayIso } from '../../prescription/rx-labels';

/** Tambah/ubah pasien. Dipakai di Master Data › Pasien dan langsung dari form resep. */
@Component({
  selector: 'app-patient-dialog',
  imports: [FormsModule, ButtonModule, DialogModule, InputTextModule, SelectButtonModule, TagModule, ToggleSwitchModule],
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
          <span class="dlg-icon"><i class="pi pi-heart"></i></span>
          <div class="dlg-title">
            <span class="dlg-name">{{ patient() ? form.name || 'Pasien' : 'Pasien baru' }}</span>
            @if (patient(); as p) {
              <span class="dlg-meta">
                <span class="mono">{{ p.code }}</span>
                <p-tag [value]="form.isActive ? 'Aktif' : 'Nonaktif'" [severity]="form.isActive ? 'success' : 'secondary'" />
              </span>
            } @else {
              <span class="dlg-meta">Isi data pasien; kode dibuat otomatis.</span>
            }
          </div>
        </div>
      </ng-template>

      <section class="card">
        <header class="card-head">
          <span class="card-icon"><i class="pi pi-id-card"></i></span>
          <div>
            <h3>Data pasien</h3>
            <p>Nama, umur, dan alamat otomatis terisi saat pasien dipilih di resep.</p>
          </div>
        </header>
        <div class="grid">
          <div class="field span-2">
            <label for="ptName">Nama <span class="req">*</span></label>
            <input pInputText id="ptName" [(ngModel)]="form.name" autofocus />
          </div>
          <div class="field">
            <label>Jenis kelamin</label>
            <p-selectbutton
              [options]="genders"
              optionLabel="label"
              optionValue="value"
              [(ngModel)]="form.gender"
              [allowEmpty]="true"
            />
          </div>
          <div class="field">
            <label for="ptBirth">Tanggal lahir</label>
            <input pInputText id="ptBirth" type="date" [max]="today" [(ngModel)]="birthDate" />
            @if (age(); as a) {
              <small>Umur {{ a }}</small>
            }
          </div>
          <div class="field">
            <label for="ptPhone">Telepon</label>
            <input pInputText id="ptPhone" [(ngModel)]="form.phone" />
          </div>
          <div class="field">
            <label for="ptAddr">Alamat</label>
            <input pInputText id="ptAddr" [(ngModel)]="form.address" />
          </div>
        </div>
      </section>

      <label class="switch-row">
        <span>
          Aktif
          <small>Bisa dipilih saat input resep.</small>
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
export class PatientDialog implements OnInit {
  private readonly notify = inject(Notify);

  /** `null` = pasien baru. */
  readonly patient = input<Patient | null>(null);
  /** Data awal untuk pasien baru (misal dari isian form resep). */
  readonly initial = input<Partial<PatientInput>>({});
  readonly saved = output<Patient>();
  readonly closed = output<void>();

  protected readonly genders = GENDERS;
  protected readonly today = todayIso();
  protected readonly saving = signal(false);
  private readonly _birthDate = signal('');
  protected readonly age = computed(() => ageFromBirthDate(this._birthDate()));
  protected form: PatientInput = {
    id: null,
    name: '',
    gender: null,
    birthDate: null,
    address: null,
    phone: null,
    isActive: true,
  };

  protected get birthDate(): string {
    return this._birthDate();
  }
  protected set birthDate(value: string) {
    this._birthDate.set(value ?? '');
  }

  ngOnInit(): void {
    const p = this.patient();
    this.form = p
      ? {
          id: p.id,
          name: p.name,
          gender: p.gender,
          birthDate: p.birthDate,
          address: p.address,
          phone: p.phone,
          isActive: p.isActive,
        }
      : { ...this.form, ...this.initial() };
    this._birthDate.set(this.form.birthDate ?? '');
  }

  protected async save(): Promise<void> {
    this.saving.set(true);
    try {
      const patient = await prescriptionApi.patientSave({ ...this.form, birthDate: this._birthDate() || null });
      this.notify.success(`Pasien ${patient.name} tersimpan`);
      this.saved.emit(patient);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }
}
