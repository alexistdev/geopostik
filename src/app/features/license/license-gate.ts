import { Component, inject, signal } from '@angular/core';
import { RouterLink } from '@angular/router';
import { ButtonModule } from 'primeng/button';

import { AuthService } from '../../core/auth/auth.service';
import { LicenseService } from '../../core/license/license.service';
import { Notify } from '../../core/ui/notify';

/** Kotak yang menggantikan isi halaman saat license tidak aktif (lihat layout/shell). */
@Component({
  selector: 'app-license-gate',
  imports: [RouterLink, ButtonModule],
  template: `
    <div class="gate">
      <i class="pi pi-lock"></i>
      <h2>License tidak aktif</h2>
      @if (license.status()?.message; as message) {
        <p class="reason">{{ message }}</p>
      }
      <p>
        Masukkan license anda, dapatkan license di
        <a href="#" (click)="$event.preventDefault(); license.openPortal()">{{ license.status()?.portalUrl }}</a>
      </p>
      <div class="actions">
        <p-button label="Validasi Ulang" icon="pi pi-refresh" [loading]="loading()" (onClick)="revalidate()" />
        @if (auth.can('SETTINGS_MANAGE')) {
          <p-button label="Buka Pengaturan License" icon="pi pi-cog" severity="secondary" [outlined]="true" routerLink="/pengaturan" />
        }
      </div>
      <small>Validasi ulang membutuhkan koneksi internet.</small>
      @if (!auth.can('SETTINGS_MANAGE')) {
        <small>Hubungi pemilik apotek untuk memperpanjang atau mengganti license.</small>
      }
    </div>
  `,
  styles: `
    .gate {
      max-width: 520px;
      margin: 3rem auto;
      padding: 2rem;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 0.5rem;
      text-align: center;
      background: var(--p-surface-0);
      border: 1px solid var(--p-surface-200);
      border-radius: 12px;
    }
    .gate > i {
      font-size: 2.5rem;
      color: var(--p-orange-500);
    }
    h2 {
      margin: 0.5rem 0 0;
    }
    p {
      margin: 0;
    }
    .reason {
      color: var(--p-red-600);
    }
    .actions {
      display: flex;
      gap: 0.5rem;
      margin: 1rem 0 0.5rem;
    }
    small {
      color: var(--p-text-muted-color);
    }
  `,
})
export class LicenseGate {
  protected readonly license = inject(LicenseService);
  protected readonly auth = inject(AuthService);
  private readonly notify = inject(Notify);
  protected readonly loading = signal(false);

  protected async revalidate(): Promise<void> {
    this.loading.set(true);
    try {
      await this.license.revalidate();
      if (this.license.active()) {
        this.notify.success('License berhasil divalidasi');
      }
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }
}
