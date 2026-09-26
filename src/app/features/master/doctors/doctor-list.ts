import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';

import type { Doctor } from '../../../bindings/Doctor';
import { prescriptionApi } from '../../../core/api/prescription.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { DateTimePipe } from '../../../shared/format';
import { MasterRowActions, type MasterRowApi } from '../master-row-actions';
import { MASTER_PAGE_SIZES, MasterTable, PAGE_REPORT } from '../master-table';
import { DoctorDialog } from './doctor-dialog';

/** Master Dokter: penulis resep, dipilih saat input resep. */
@Component({
  selector: 'app-doctor-list',
  imports: [
    FormsModule,
    ButtonModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    TableModule,
    DateTimePipe,
    MasterRowActions,
    DoctorDialog,
  ],
  templateUrl: './doctor-list.html',
  styleUrl: '../people-list.scss',
})
export class DoctorList {
  private readonly notify = inject(Notify);
  protected readonly canEdit = inject(AuthService).can('PRESCRIPTION_INPUT');

  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected readonly table = new MasterTable<Doctor>(
    prescriptionApi.doctorPage,
    prescriptionApi.doctorList,
    (e) => this.notify.error(e),
  );
  protected readonly rowApi: MasterRowApi = {
    setActive: prescriptionApi.doctorSetActive,
    remove: prescriptionApi.doctorDelete,
  };
  /** `undefined` = dialog tertutup, `null` = dokter baru. */
  protected readonly editing = signal<Doctor | null | undefined>(undefined);

  constructor() {
    this.table.load(0);
  }

  protected open(doctor: Doctor | null): void {
    if (this.canEdit) this.editing.set(doctor);
  }

  protected saved(): void {
    this.editing.set(undefined);
    this.table.load();
  }
}
