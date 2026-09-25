import { Injectable, computed, signal } from '@angular/core';

import type { LoginInput } from '../../bindings/LoginInput';
import type { Permission } from '../../bindings/Permission';
import type { Role } from '../../bindings/Role';
import type { SessionUser } from '../../bindings/SessionUser';
import type { SetupInput } from '../../bindings/SetupInput';
import { authApi } from '../api/auth.api';

export const ROLE_LABELS: Record<Role, string> = {
  OWNER: 'Pemilik',
  PHARMACIST: 'Apoteker',
  TECHNICIAN: 'TTK',
  CASHIER: 'Kasir',
};

/**
 * Status login di sisi tampilan. Sesi sebenarnya disimpan di Rust;
 * permission di sini hanya untuk menampilkan/menyembunyikan menu.
 */
@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly _user = signal<SessionUser | null>(null);
  private readonly _needsSetup = signal(false);
  private status: Promise<void> | null = null;

  readonly user = this._user.asReadonly();
  readonly needsSetup = this._needsSetup.asReadonly();
  readonly roleLabel = computed(() =>
    (this._user()?.roles ?? []).map((r) => ROLE_LABELS[r]).join(', '),
  );

  /** Memuat status dari Rust sekali saat aplikasi dibuka. */
  loadStatus(): Promise<void> {
    this.status ??= authApi
      .appStatus()
      .then((s) => {
        this._needsSetup.set(s.needsSetup);
        this._user.set(s.session);
      })
      .catch((e) => {
        this.status = null; // boleh dicoba lagi
        throw e;
      });
    return this.status;
  }

  async setupOwner(input: SetupInput): Promise<void> {
    const user = await authApi.setupOwner(input);
    this._needsSetup.set(false);
    this._user.set(user);
  }

  async login(input: LoginInput): Promise<void> {
    this._user.set(await authApi.login(input));
  }

  async logout(): Promise<void> {
    await authApi.logout();
    this._user.set(null);
  }

  can(permission: Permission): boolean {
    return this._user()?.permissions.includes(permission) ?? false;
  }
}
