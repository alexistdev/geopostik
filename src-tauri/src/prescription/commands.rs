use tauri::State;

use super::model::{
    Doctor, DoctorInput, DoctorPage, Patient, PatientInput, PatientPage, PrescriptionDetail, PrescriptionInput,
    PrescriptionPage, PrescriptionQuery, ScreeningInput,
};
use super::repo::PersonTable;
use super::service;
use crate::auth::Permission;
use crate::error::AppResult;
use crate::master::MasterPageQuery;
use crate::state::AppState;

// Dokter & pasien dikelola oleh yang berhak input resep (pemilik, apoteker, TTK), dari menu
// Master Data maupun langsung dari form resep.

#[tauri::command]
pub async fn doctor_list(state: State<'_, AppState>) -> AppResult<Vec<Doctor>> {
    state.current_user()?;
    service::list_doctors(&*state.db()?)
}

#[tauri::command]
pub async fn doctor_page(state: State<'_, AppState>, query: MasterPageQuery) -> AppResult<DoctorPage> {
    state.current_user()?;
    service::page_doctors(&*state.db()?, &query)
}

#[tauri::command]
pub async fn doctor_save(state: State<'_, AppState>, input: DoctorInput) -> AppResult<Doctor> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::save_doctor(&*state.db()?, &user, &input)
}

#[tauri::command]
pub async fn doctor_set_active(state: State<'_, AppState>, id: i64, active: bool) -> AppResult<()> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::set_person_active(&*state.db()?, &user, PersonTable::Doctors, id, active)
}

#[tauri::command]
pub async fn doctor_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::delete_person(&mut *state.db()?, &user, PersonTable::Doctors, id)
}

/// `active_only` untuk pencarian pasien di form resep (yang nonaktif tidak bisa dipilih).
#[tauri::command]
pub async fn patient_page(state: State<'_, AppState>, query: MasterPageQuery, active_only: bool) -> AppResult<PatientPage> {
    state.current_user()?;
    service::page_patients(&*state.db()?, &query, active_only)
}

#[tauri::command]
pub async fn patient_save(state: State<'_, AppState>, input: PatientInput) -> AppResult<Patient> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::save_patient(&*state.db()?, &user, &input)
}

#[tauri::command]
pub async fn patient_set_active(state: State<'_, AppState>, id: i64, active: bool) -> AppResult<()> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::set_person_active(&*state.db()?, &user, PersonTable::Patients, id, active)
}

#[tauri::command]
pub async fn patient_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::delete_person(&mut *state.db()?, &user, PersonTable::Patients, id)
}

#[tauri::command]
pub async fn prescription_page(state: State<'_, AppState>, query: PrescriptionQuery) -> AppResult<PrescriptionPage> {
    state.require(Permission::PrescriptionInput)?;
    service::page_prescriptions(&*state.db()?, &query)
}

#[tauri::command]
pub async fn prescription_get(state: State<'_, AppState>, id: i64) -> AppResult<PrescriptionDetail> {
    state.require(Permission::PrescriptionInput)?;
    service::get_prescription(&*state.db()?, id)
}

#[tauri::command]
pub async fn prescription_save(state: State<'_, AppState>, input: PrescriptionInput) -> AppResult<PrescriptionDetail> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::save_prescription(&mut *state.db()?, &user, &input)
}

/// Skrining/validasi resep: hanya apoteker.
#[tauri::command]
pub async fn prescription_screen(state: State<'_, AppState>, input: ScreeningInput) -> AppResult<PrescriptionDetail> {
    let user = state.require(Permission::PrescriptionValidate)?;
    service::screen_prescription(&mut *state.db()?, &user, &input)
}

#[tauri::command]
pub async fn prescription_cancel(state: State<'_, AppState>, id: i64, reason: String) -> AppResult<PrescriptionDetail> {
    let user = state.require(Permission::PrescriptionInput)?;
    service::cancel_prescription(&mut *state.db()?, &user, id, &reason)
}
