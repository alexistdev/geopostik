import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { MessageModule } from 'primeng/message';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';

import type { LicenseState } from '../../bindings/LicenseState';
import { settingsApi } from '../../core/api/settings.api';
import { LicenseService } from '../../core/license/license.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, bpToPercent, percentToBp } from '../../shared/format';

const STATE_LABELS: Record<LicenseState, { label: string; severity: 'success' | 'danger' | 'warn' }> = {
  NOT_ACTIVATED: { label: 'Belum diaktifkan', severity: 'danger' },
  ACTIVE: { label: 'Aktif', severity: 'success' },
  EXPIRED: { label: 'Kedaluwarsa', severity: 'danger' },
  CLOCK_ROLLBACK: { label: 'Jam komputer mundur', severity: 'warn' },
};

/** Menu Pengaturan. Tetap bisa dibuka saat license tidak aktif. */
@Component({
  selector: 'app-settings',
  imports: [FormsModule, ButtonModule, InputNumberModule, InputTextModule, MessageModule, TagModule, ToggleSwitchModule, DateTimePipe],
  template: `
    <h2>Pengaturan</h2>

    <section class="card">
      <header>
        <h3>License</h3>
        @if (stateLabel(); as s) {
          <p-tag [value]="s.label" [severity]="s.severity" />
        }
      </header>

      @if (status(); as st) {
        @if (st.message) {
          <p-message severity="warn">{{ st.message }}</p-message>
        }

        <dl>
          <dt>License key</dt>
          <dd class="mono">{{ st.licenseKey ?? '–' }}</dd>
          <dt>Aktif sampai</dt>
          <dd>
            {{ st.expiresAt | dateTime }}
            @if (st.state === 'ACTIVE' && st.daysLeft != null) {
              <span class="muted">(sisa {{ st.daysLeft }} hari)</span>
            }
          </dd>
          <dt>Diaktifkan</dt>
          <dd>{{ st.activatedAt | dateTime }}</dd>
          <dt>Validasi terakhir</dt>
          <dd>{{ st.validatedAt | dateTime }}</dd>
          <dt>Komputer terpakai</dt>
          <dd>{{ st.usedSeats ?? '–' }} dari {{ st.maxSeats ?? '–' }}</dd>
          <dt>ID komputer</dt>
          <dd class="mono">{{ st.machineId }}</dd>
        </dl>

        <p class="muted">
          License disimpan di komputer ini dan tidak butuh internet sampai masa aktifnya habis.
          Setelah memperpanjang license di
          <a href="#" (click)="$event.preventDefault(); license.openPortal()">{{ st.portalUrl }}</a>,
          klik <b>Validasi Ulang</b> saat komputer terhubung ke internet.
        </p>

        <div class="actions">
          <p-button label="Validasi Ulang" icon="pi pi-refresh" [loading]="loading() === 'revalidate'" [disabled]="!!loading()" (onClick)="revalidate()" />
        </div>

        <form class="change" (ngSubmit)="activate()">
          <label for="licenseKey">Ganti license key</label>
          <div class="row">
            <input pInputText id="licenseKey" name="licenseKey" placeholder="GEOLIC-XXXXXXXX-XXXXXXXX" autocomplete="off" [(ngModel)]="newKey" />
            <p-button type="submit" label="Aktifkan" icon="pi pi-key" severity="secondary" [loading]="loading() === 'activate'" [disabled]="!!loading() || !newKey.trim()" />
          </div>
          <small class="muted">Butuh internet. License lama tetap dipakai bila license baru ditolak.</small>
        </form>
      }
    </section>

    @if (tax(); as t) {
      <section class="card">
        <header>
          <h3>Pajak pembelian</h3>
        </header>
        <label class="switch">
          <p-toggleswitch [(ngModel)]="t.isPkp" />
          <span>
            <b>Apotek PKP</b> (Pengusaha Kena Pajak)
            <small class="muted">
              PKP: PPN faktur pembelian dikreditkan sehingga tidak masuk HPP. Non-PKP: PPN ikut menjadi HPP.
            </small>
          </span>
        </label>
        <div class="change">
          <label for="ppnRate">Tarif PPN default faktur</label>
          <div class="row">
            <p-inputnumber inputId="ppnRate" [(ngModel)]="t.rate" suffix="%" [min]="0" [max]="100" [maxFractionDigits]="2" locale="id-ID" />
            <p-button label="Simpan" icon="pi pi-check" severity="secondary" [loading]="savingTax()" (onClick)="saveTax()" />
          </div>
          <small class="muted">Bisa diubah per faktur. Faktur yang sudah diposting tetap memakai pengaturan saat diposting.</small>
        </div>
      </section>
    }
  `,
  styles: `
    h2 {
      margin-top: 0;
    }
    .card {
      max-width: 640px;
      padding: 1.25rem 1.5rem;
      background: var(--p-surface-0);
      border: 1px solid var(--p-surface-200);
      border-radius: 8px;
    }
    header {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      margin-bottom: 1rem;
    }
    h3 {
      margin: 0;
    }
    p-message {
      display: block;
      margin-bottom: 1rem;
    }
    dl {
      display: grid;
      grid-template-columns: 160px 1fr;
      gap: 0.5rem 1rem;
      margin: 0 0 1rem;
    }
    dt {
      color: var(--p-text-muted-color);
    }
    dd {
      margin: 0;
    }
    .card + .card {
      margin-top: 1rem;
    }
    .switch {
      display: flex;
      align-items: flex-start;
      gap: 0.75rem;
      margin-bottom: 1rem;
      cursor: pointer;
    }
    .switch span {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
    }
    .mono {
      font-family: Consolas, monospace;
      user-select: text;
    }
    .muted {
      color: var(--p-text-muted-color);
    }
    .actions {
      margin-bottom: 1.5rem;
    }
    .change {
      display: flex;
      flex-direction: column;
      gap: 0.375rem;
      padding-top: 1rem;
      border-top: 1px solid var(--p-surface-200);
    }
    .change label {
      font-weight: 600;
    }
    .row {
      display: flex;
      gap: 0.5rem;
    }
    .row input {
      flex: 1;
    }
  `,
})
export class Settings {
  protected readonly license = inject(LicenseService);
  private readonly notify = inject(Notify);

  protected readonly status = this.license.status;
  protected readonly stateLabel = computed(() => {
    const state = this.status()?.state;
    return state ? STATE_LABELS[state] : null;
  });
  protected readonly loading = signal<'revalidate' | 'activate' | null>(null);
  protected newKey = '';
  /** Pajak; kosong bila tidak bisa dimuat (misal license tidak aktif atau bukan pemilik). */
  protected readonly tax = signal<{ isPkp: boolean; rate: number | null } | null>(null);
  protected readonly savingTax = signal(false);

  constructor() {
    settingsApi.taxGet().then(
      (t) => this.tax.set({ isPkp: t.isPkp, rate: bpToPercent(t.ppnRateBp) }),
      () => this.tax.set(null),
    );
  }

  protected async saveTax(): Promise<void> {
    const t = this.tax();
    if (!t) return;
    this.savingTax.set(true);
    try {
      await settingsApi.taxSave({ isPkp: t.isPkp, ppnRateBp: percentToBp(t.rate) ?? 0 });
      this.notify.success('Pengaturan pajak disimpan');
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.savingTax.set(false);
    }
  }

  protected async revalidate(): Promise<void> {
    this.loading.set('revalidate');
    try {
      await this.license.revalidate();
      this.notify.success('License berhasil divalidasi');
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(null);
    }
  }

  protected async activate(): Promise<void> {
    this.loading.set('activate');
    try {
      await this.license.activate(this.newKey);
      this.newKey = '';
      this.notify.success('License baru berhasil diaktifkan');
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(null);
    }
  }
}
