import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';

import type { Permission } from '../../bindings/Permission';
import { LicenseService } from '../license/license.service';
import { AuthService } from './auth.service';

/** Memuat status; bila backend tidak tersedia, halaman login tetap tampil dengan pesan error. */
async function status(): Promise<AuthService> {
  const auth = inject(AuthService);
  await auth.loadStatus().catch(() => undefined);
  return auth;
}

/** Aktivasi license pertama: hanya bila belum pernah diaktifkan di komputer ini. */
export const activationGuard: CanActivateFn = async () => {
  const router = inject(Router);
  const license = inject(LicenseService);
  await status();
  return license.notActivated() ? true : router.parseUrl('/');
};

/** Halaman setup awal: hanya bila belum ada user. */
export const setupGuard: CanActivateFn = async () => {
  const router = inject(Router);
  const license = inject(LicenseService);
  const auth = await status();
  if (license.notActivated()) return router.parseUrl('/aktivasi');
  return auth.needsSetup() ? true : router.parseUrl('/login');
};

/** Halaman login: hanya bila belum login. */
export const guestGuard: CanActivateFn = async () => {
  const router = inject(Router);
  const license = inject(LicenseService);
  const auth = await status();
  if (license.notActivated()) return router.parseUrl('/aktivasi');
  if (auth.needsSetup()) return router.parseUrl('/setup');
  return auth.user() ? router.parseUrl('/') : true;
};

/** Semua halaman di dalam aplikasi: wajib login. */
export const authGuard: CanActivateFn = async () => {
  const router = inject(Router);
  const license = inject(LicenseService);
  const auth = await status();
  if (license.notActivated()) return router.parseUrl('/aktivasi');
  if (auth.needsSetup()) return router.parseUrl('/setup');
  return auth.user() ? true : router.parseUrl('/login');
};

/** Halaman yang butuh hak tertentu. */
export function permissionGuard(permission: Permission): CanActivateFn {
  return () => {
    const router = inject(Router);
    return inject(AuthService).can(permission) ? true : router.parseUrl('/');
  };
}
