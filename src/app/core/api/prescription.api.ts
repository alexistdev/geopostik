import type { Doctor } from '../../bindings/Doctor';
import type { DoctorInput } from '../../bindings/DoctorInput';
import type { DoctorPage } from '../../bindings/DoctorPage';
import type { MasterPageQuery } from '../../bindings/MasterPageQuery';
import type { Patient } from '../../bindings/Patient';
import type { PatientInput } from '../../bindings/PatientInput';
import type { PatientPage } from '../../bindings/PatientPage';
import type { PrescriptionDetail } from '../../bindings/PrescriptionDetail';
import type { PrescriptionInput } from '../../bindings/PrescriptionInput';
import type { PrescriptionPage } from '../../bindings/PrescriptionPage';
import type { PrescriptionQuery } from '../../bindings/PrescriptionQuery';
import type { ScreeningInput } from '../../bindings/ScreeningInput';
import { call } from './tauri';

export const prescriptionApi = {
  doctorList: () => call<Doctor[]>('doctor_list'),
  doctorPage: (query: MasterPageQuery) => call<DoctorPage>('doctor_page', { query }),
  doctorSave: (input: DoctorInput) => call<Doctor>('doctor_save', { input }),
  doctorSetActive: (id: number, active: boolean) => call<void>('doctor_set_active', { id, active }),
  doctorDelete: (id: number) => call<void>('doctor_delete', { id }),
  patientPage: (query: MasterPageQuery, activeOnly = false) =>
    call<PatientPage>('patient_page', { query, activeOnly }),
  patientSave: (input: PatientInput) => call<Patient>('patient_save', { input }),
  patientSetActive: (id: number, active: boolean) => call<void>('patient_set_active', { id, active }),
  patientDelete: (id: number) => call<void>('patient_delete', { id }),
  page: (query: PrescriptionQuery) => call<PrescriptionPage>('prescription_page', { query }),
  get: (id: number) => call<PrescriptionDetail>('prescription_get', { id }),
  save: (input: PrescriptionInput) => call<PrescriptionDetail>('prescription_save', { input }),
  screen: (input: ScreeningInput) => call<PrescriptionDetail>('prescription_screen', { input }),
  cancel: (id: number, reason: string) => call<PrescriptionDetail>('prescription_cancel', { id, reason }),
};
