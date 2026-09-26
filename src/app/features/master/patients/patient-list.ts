import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';

import type { Patient } from '../../../bindings/Patient';
import { prescriptionApi } from '../../../core/api/prescription.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { DateTimePipe } from '../../../shared/format';
import { ageFromBirthDate, formatDate, genderLabel } from '../../prescription/rx-labels';
import { MasterRowActions, type MasterRowApi } from '../master-row-actions';
import { MASTER_PAGE_SIZES, MasterTable, PAGE_REPORT } from '../master-table';
import { PatientDialog } from './patient-dialog';

/** Master Pasien (tabel `customers`): dipilih saat input resep, juga untuk data pelanggan. */
@Component({
  selector: 'app-patient-list',
  imports: [
    FormsModule,
    ButtonModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    TableModule,
    DateTimePipe,
    MasterRowActions,
    PatientDialog,
  ],
  templateUrl: './patient-list.html',
  styleUrl: '../people-list.scss',
})
export class PatientList {
  private readonly notify = inject(Notify);
  protected readonly canEdit = inject(AuthService).can('PRESCRIPTION_INPUT');

  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected readonly genderLabel = genderLabel;
  protected readonly formatDate = formatDate;
  protected readonly age = ageFromBirthDate;
  protected readonly table = new MasterTable<Patient>(
    (query) => prescriptionApi.patientPage(query),
    // Pasien tidak dicetak sebagai label.
    async () => [],
    (e) => this.notify.error(e),
  );
  protected readonly rowApi: MasterRowApi = {
    setActive: prescriptionApi.patientSetActive,
    remove: prescriptionApi.patientDelete,
  };
  /** `undefined` = dialog tertutup, `null` = pasien baru. */
  protected readonly editing = signal<Patient | null | undefined>(undefined);

  constructor() {
    this.table.load(0);
  }

  protected open(patient: Patient | null): void {
    if (this.canEdit) this.editing.set(patient);
  }

  protected saved(): void {
    this.editing.set(undefined);
    this.table.load();
  }
}
