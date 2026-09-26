import { Component, effect, inject, model, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { PasswordModule } from 'primeng/password';
import { TagModule } from 'primeng/tag';

import type { SaleDetail } from '../../bindings/SaleDetail';
import type { SaleRow } from '../../bindings/SaleRow';
import { salesApi } from '../../core/api/sales.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe } from '../../shared/format';
import { Receipt } from './receipt';

const METHOD: Record<string, string> = { CASH: 'Tunai', QRIS: 'QRIS', DEBIT: 'Debit' };

/** Riwayat nota shift yang sedang terbuka, lihat ulang struk, dan batalkan (void) dengan otorisasi. */
@Component({
  selector: 'app-sales-history',
  imports: [FormsModule, ButtonModule, DialogModule, InputTextModule, PasswordModule, TagModule, RupiahPipe, DateTimePipe, Receipt],
  template: `
    <p-dialog
      header="Riwayat nota shift ini"
      [(visible)]="open"
      [modal]="true"
      [style]="{ width: '980px', maxWidth: '96vw' }"
      [draggable]="false"
      (onShow)="load()"
    >
      <div class="history">
        <div class="list">
          <input pInputText type="search" placeholder="Cari nomor nota" [(ngModel)]="q" (ngModelChange)="load()" class="w-full" />
          <div class="rows">
            @for (r of rows(); track r.id) {
              <button type="button" class="row" [class.active]="selected()?.id === r.id" [class.void]="r.status === 'VOID'" (click)="select(r)">
                <span class="main">
                  <span class="mono">{{ r.number }}</span>
                  <small>{{ r.soldAt | dateTime }} · {{ r.cashierName }} · {{ r.itemCount }} item</small>
                </span>
                <span class="side">
                  <strong>{{ r.grandTotal | rupiah }}</strong>
                  @if (r.status === 'VOID') {
                    <p-tag value="Batal" severity="danger" />
                  } @else {
                    <small>{{ methods(r.methods) }}{{ r.saleType === 'PRESCRIPTION' ? ' · Resep' : '' }}</small>
                  }
                </span>
              </button>
            } @empty {
              <p class="empty">{{ loading() ? 'Memuat…' : 'Belum ada nota di shift ini.' }}</p>
            }
          </div>
          @if (total() > rows().length) {
            <small class="more">Menampilkan {{ rows().length }} dari {{ total() }} nota terbaru</small>
          }
        </div>
        <div class="detail">
          @if (selected(); as s) {
            <app-receipt [sale]="s" />
            @if (s.status === 'COMPLETED') {
              <div class="void-box">
                <strong><i class="pi pi-ban"></i> Batalkan nota</strong>
                <input pInputText placeholder="Alasan pembatalan (wajib)" [(ngModel)]="reason" maxlength="200" />
                @if (!canVoid) {
                  <p-password [(ngModel)]="pin" [feedback]="false" placeholder="PIN Pemilik/Apoteker" [toggleMask]="true" inputStyleClass="w-full" styleClass="w-full" />
                }
                <p-button
                  label="Batalkan nota"
                  icon="pi pi-ban"
                  severity="danger"
                  [loading]="voiding()"
                  [disabled]="!reason.trim() || (!canVoid && !pin.trim())"
                  (onClick)="voidSale(s)"
                />
                <small>Stok dikembalikan ke batch asal. Hanya nota dari shift yang masih terbuka.</small>
              </div>
            }
          } @else {
            <p class="empty">Pilih nota untuk melihat struk.</p>
          }
        </div>
      </div>
    </p-dialog>
  `,
  styles: `
    .history { display: grid; grid-template-columns: minmax(0, 1fr) 340px; gap: 1.25rem; min-height: 460px; }
    .list { display: flex; flex-direction: column; gap: 0.6rem; min-width: 0; }
    .rows { flex: 1; max-height: 520px; overflow: auto; border: 1px solid var(--p-content-border-color); border-radius: 10px; }
    .row {
      display: flex; justify-content: space-between; gap: 0.75rem; width: 100%; padding: 0.6rem 0.8rem;
      text-align: left; font: inherit; color: inherit; background: none; border: 0;
      border-bottom: 1px solid var(--p-content-border-color); cursor: pointer;
    }
    .row:hover { background: var(--p-surface-50); }
    .row.active { background: var(--p-primary-50); }
    .row.void .main .mono { text-decoration: line-through; opacity: 0.7; }
    .main, .side { display: flex; flex-direction: column; gap: 0.15rem; }
    .side { align-items: flex-end; }
    .mono { font-family: ui-monospace, Consolas, monospace; font-size: 0.85rem; }
    small { color: var(--p-text-muted-color); }
    .empty { padding: 1rem; color: var(--p-text-muted-color); text-align: center; }
    .more { text-align: center; }
    .detail { display: flex; flex-direction: column; gap: 1rem; }
    .void-box {
      display: flex; flex-direction: column; gap: 0.5rem; padding: 0.85rem;
      border: 1px solid var(--p-red-200); border-radius: 10px; background: var(--p-red-50);
    }
    .void-box strong { color: var(--p-red-700); }
  `,
})
export class SalesHistory {
  readonly open = model(false);
  /** Nota dibatalkan: layar kasir perlu menyegarkan angka shift. */
  readonly changed = output<void>();

  private readonly notify = inject(Notify);
  protected readonly canVoid = inject(AuthService).can('TRANSACTION_VOID');

  protected q = '';
  protected reason = '';
  protected pin = '';
  protected readonly rows = signal<SaleRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);
  protected readonly selected = signal<SaleDetail | null>(null);
  protected readonly voiding = signal(false);

  constructor() {
    effect(() => {
      if (!this.open()) {
        this.selected.set(null);
        this.reason = '';
        this.pin = '';
      }
    });
  }

  protected methods(m: string): string {
    return m
      .split('+')
      .filter(Boolean)
      .map((x) => METHOD[x] ?? x)
      .join(' + ');
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      const page = await salesApi.page({ shiftId: null, q: this.q.trim() || null, offset: 0, limit: 100 });
      this.rows.set(page.rows);
      this.total.set(page.total);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected async select(r: SaleRow): Promise<void> {
    try {
      this.selected.set(await salesApi.get(r.id));
      this.reason = '';
      this.pin = '';
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected async voidSale(s: SaleDetail): Promise<void> {
    if (this.voiding()) return;
    this.voiding.set(true);
    try {
      const done = await salesApi.void({ saleId: s.id, reason: this.reason.trim(), pin: this.canVoid ? null : this.pin.trim() });
      this.selected.set(done);
      this.notify.success(`Nota ${done.number} dibatalkan, stok dikembalikan`);
      this.changed.emit();
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.pin = '';
      this.voiding.set(false);
    }
  }
}
