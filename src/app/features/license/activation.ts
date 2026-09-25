import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { MessageModule } from 'primeng/message';

import { toApiError } from '../../core/api/tauri';
import { LicenseService } from '../../core/license/license.service';

/** Aktivasi license saat aplikasi pertama kali dipasang. Butuh internet sekali ini saja. */
@Component({
  selector: 'app-activation',
  imports: [FormsModule, ButtonModule, InputTextModule, MessageModule],
  template: `
    <form class="card" (ngSubmit)="submit()">
      <h1 class="title">Aktivasi GeoPOSTik</h1>
      <p class="subtitle">
        Masukkan license anda, dapatkan license di
        <a href="#" (click)="$event.preventDefault(); license.openPortal()">{{ portalUrl() }}</a>
      </p>

      @if (error(); as message) {
        <p-message severity="error">{{ message }}</p-message>
      }

      <div class="field">
        <label for="licenseKey">License key</label>
        <input
          pInputText
          id="licenseKey"
          name="licenseKey"
          placeholder="GEOLIC-XXXXXXXX-XXXXXXXX"
          [(ngModel)]="licenseKey"
          autofocus
          autocomplete="off"
        />
        <small>Komputer harus terhubung ke internet saat aktivasi. Setelah itu aplikasi berjalan offline.</small>
      </div>

      <p-button type="submit" label="Aktifkan" icon="pi pi-key" [loading]="loading()" [fluid]="true" />

      <p class="machine">ID komputer: {{ license.status()?.machineId }}</p>
    </form>
  `,
  styleUrl: '../auth/auth-page.scss',
  styles: `
    .machine {
      margin: 1rem 0 0;
      font-size: 0.8rem;
      color: var(--p-text-muted-color);
      text-align: center;
      user-select: text;
    }
  `,
})
export class Activation {
  protected readonly license = inject(LicenseService);
  private readonly router = inject(Router);

  protected licenseKey = '';
  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly portalUrl = () => this.license.status()?.portalUrl ?? 'https://geolicense.my.id/';

  protected async submit(): Promise<void> {
    if (this.loading()) return;
    this.loading.set(true);
    this.error.set(null);
    try {
      await this.license.activate(this.licenseKey);
      await this.router.navigateByUrl('/');
    } catch (e) {
      this.error.set(toApiError(e).message);
    } finally {
      this.loading.set(false);
    }
  }
}
