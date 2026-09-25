import { Component, OnInit, computed, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';

import type { Role } from '../../bindings/Role';
import type { UserRow } from '../../bindings/UserRow';
import { ApiError } from '../../core/api/tauri';
import { userApi } from '../../core/api/user.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { ROLES } from '../../shared/labels';

interface Form {
  username: string;
  fullName: string;
  roles: Role[];
  licenseNumber: string;
  password: string;
  passwordConfirm: string;
  pin: string;
  isActive: boolean;
}

/** Tambah / ubah pengguna: data akun, peran, nomor izin, password & PIN. */
@Component({
  selector: 'app-user-dialog',
  imports: [FormsModule, ButtonModule, DialogModule, InputTextModule, TagModule, ToggleSwitchModule],
  templateUrl: './user-dialog.html',
  styleUrl: './user-dialog.scss',
})
export class UserDialog implements OnInit {
  private readonly notify = inject(Notify);
  private readonly auth = inject(AuthService);

  /** `null` = pengguna baru. */
  readonly user = input<UserRow | null>(null);
  readonly closed = output<boolean>();

  protected readonly allRoles = ROLES;
  protected readonly visible = signal(true);
  protected readonly saving = signal(false);
  protected readonly isNew = computed(() => !this.user());
  protected readonly isSelf = computed(() => this.user()?.id === this.auth.user()?.id);
  protected form: Form = {
    username: '',
    fullName: '',
    roles: [],
    licenseNumber: '',
    password: '',
    passwordConfirm: '',
    pin: '',
    isActive: true,
  };

  ngOnInit(): void {
    const u = this.user();
    if (u) {
      this.form = {
        ...this.form,
        username: u.username,
        fullName: u.fullName,
        roles: [...u.roles],
        licenseNumber: u.licenseNumber ?? '',
        isActive: u.isActive,
      };
    }
  }

  protected hasRole(role: Role): boolean {
    return this.form.roles.includes(role);
  }

  protected toggleRole(role: Role): void {
    if (this.roleLocked(role)) return;
    const roles = this.form.roles;
    const next = roles.includes(role) ? roles.filter((r) => r !== role) : [...roles, role];
    // Urutan sama dengan daftar peran (kewenangan tertinggi dulu).
    this.form.roles = ROLES.map((r) => r.value).filter((r) => next.includes(r));
  }

  /** Peran Pemilik di akun sendiri tidak bisa dilepas (agar tidak terkunci dari menu ini). */
  protected roleLocked(role: Role): boolean {
    return role === 'OWNER' && this.isSelf() && (this.user()?.roles.includes('OWNER') ?? false);
  }

  /** Nomor izin: SIPA untuk apoteker (wajib), SIPTTK untuk TTK (opsional). */
  protected license(): { label: string; required: boolean } | null {
    if (this.hasRole('PHARMACIST')) return { label: 'Nomor SIPA', required: true };
    if (this.hasRole('TECHNICIAN')) return { label: 'Nomor SIPTTK', required: false };
    return null;
  }

  protected passwordMismatch(): boolean {
    return this.form.passwordConfirm !== '' && this.form.password !== this.form.passwordConfirm;
  }

  protected async save(): Promise<void> {
    const f = this.form;
    if (f.password !== f.passwordConfirm) {
      this.notify.error(new ApiError('VALIDATION', 'Ulangi password tidak sama'));
      return;
    }
    this.saving.set(true);
    try {
      const result = await userApi.save({
        id: this.user()?.id ?? null,
        username: f.username,
        fullName: f.fullName,
        roles: f.roles,
        licenseNumber: this.license() ? f.licenseNumber : null,
        password: f.password || null,
        pin: f.pin || null,
        isActive: f.isActive,
      });
      if (result.session) {
        this.auth.updateSession(result.session);
      }
      this.notify.success(`Pengguna ${result.user.fullName} tersimpan`);
      this.visible.set(false);
      this.closed.emit(true);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  protected close(): void {
    this.visible.set(false);
    this.closed.emit(false);
  }
}
