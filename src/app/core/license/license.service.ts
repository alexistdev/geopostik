import { Injectable, computed, signal } from '@angular/core';
import { openUrl } from '@tauri-apps/plugin-opener';

import type { LicenseStatus } from '../../bindings/LicenseStatus';
import { licenseApi } from '../api/license.api';

/**
 * Status license di sisi tampilan. Pemeriksaan sebenarnya dilakukan di Rust
 * (file license lokal); aktivasi & validasi ulang butuh internet.
 */
@Injectable({ providedIn: 'root' })
export class LicenseService {
  private readonly _status = signal<LicenseStatus | null>(null);

  readonly status = this._status.asReadonly();
  readonly active = computed(() => this._status()?.state === 'ACTIVE');
  /** Belum pernah diaktifkan di komputer ini: aplikasi belum bisa dipakai sama sekali. */
  readonly notActivated = computed(() => this._status()?.state === 'NOT_ACTIVATED');

  set(status: LicenseStatus): void {
    this._status.set(status);
  }

  /** Memeriksa ulang file license lokal (misal masa aktif habis saat aplikasi terbuka). */
  async refresh(): Promise<void> {
    this._status.set(await licenseApi.status());
  }

  async activate(licenseKey: string): Promise<void> {
    this._status.set(await licenseApi.activate(licenseKey));
  }

  async revalidate(): Promise<void> {
    this._status.set(await licenseApi.revalidate());
  }

  /** Membuka situs GeoLicense di browser. */
  openPortal(): void {
    const url = this._status()?.portalUrl ?? 'https://geolicense.my.id/';
    openUrl(url).catch(() => undefined);
  }
}
