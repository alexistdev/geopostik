import { Component, computed, inject, input } from '@angular/core';

import type { SaleDetail } from '../../bindings/SaleDetail';
import type { SaleItemDetail } from '../../bindings/SaleItemDetail';
import { AuthService } from '../../core/auth/auth.service';
import { DateTimePipe, RupiahPipe } from '../../shared/format';

const METHOD: Record<string, string> = { CASH: 'Tunai', QRIS: 'QRIS', DEBIT: 'Debit' };

/**
 * Pratinjau struk di layar. Cetak ke printer thermal (ESC/POS dari Rust) menyusul; tampilan ini
 * mengikuti isi struk yang akan dicetak.
 */
@Component({
  selector: 'app-receipt',
  imports: [RupiahPipe, DateTimePipe],
  template: `
    @let s = sale();
    <div class="paper" [class.void]="s.status === 'VOID'">
      <div class="center strong">{{ pharmacy() }}</div>
      <div class="center small">{{ s.number }} · {{ s.soldAt | dateTime }}</div>
      <div class="center small">Kasir: {{ s.cashierName }}</div>
      @if (s.prescriptionNumber) {
        <div class="center small">Resep {{ s.prescriptionNumber }} · {{ s.patientName }}</div>
      }
      @if (s.status === 'VOID') {
        <div class="stamp">BATAL</div>
      }
      <hr />
      @for (it of top(); track it.id) {
        <div class="item">
          <div class="desc">{{ it.description }}</div>
          <div class="row">
            @if (it.kind === 'COMPOUND') {
              <span>{{ it.qty }} racikan</span>
            } @else {
              <span>{{ it.qty }} {{ it.unitName ?? '' }} × {{ it.unitPrice | rupiah }}@if (it.tierMinQty) { <em>grosir</em> }</span>
            }
            <span>{{ it.lineTotal + it.discountAmount | rupiah }}</span>
          </div>
          @for (c of components(it); track c.id) {
            <div class="row small indent"><span>· {{ c.description }} {{ c.qty }} {{ c.unitName ?? '' }}</span></div>
          }
          @if (it.discountAmount > 0) {
            <div class="row small"><span>  Diskon</span><span>-{{ it.discountAmount | rupiah }}</span></div>
          }
        </div>
      }
      <hr />
      <div class="row"><span>Subtotal</span><span>{{ s.subtotal | rupiah }}</span></div>
      @if (s.discountTotal > 0) {
        <div class="row"><span>Diskon</span><span>-{{ s.discountTotal | rupiah }}</span></div>
      }
      @if (s.rounding !== 0) {
        <div class="row"><span>Pembulatan</span><span>{{ s.rounding | rupiah }}</span></div>
      }
      <div class="row total"><span>TOTAL</span><span>{{ s.grandTotal | rupiah }}</span></div>
      @for (p of s.payments; track p.method) {
        <div class="row">
          <span>{{ method[p.method] }}@if (p.reference) { <em>{{ p.reference }}</em> }</span>
          <span>{{ p.tendered ?? p.amount | rupiah }}</span>
        </div>
      }
      @if (s.changeAmount > 0) {
        <div class="row strong"><span>Kembali</span><span>{{ s.changeAmount | rupiah }}</span></div>
      }
      @if (s.status === 'VOID') {
        <hr />
        <div class="small">Dibatalkan {{ s.voidedAt | dateTime }} oleh {{ s.voidedBy }}: {{ s.voidReason }}</div>
      }
      <hr />
      <div class="center small">Terima kasih, semoga lekas sembuh</div>
    </div>
  `,
  styles: `
    .paper {
      position: relative;
      width: 100%;
      max-width: 320px;
      margin: 0 auto;
      padding: 1rem 1.1rem;
      font-family: ui-monospace, Consolas, monospace;
      font-size: 0.8rem;
      line-height: 1.45;
      color: #111;
      background: #fff;
      border: 1px solid var(--p-content-border-color);
      border-radius: 4px;
      box-shadow: 0 6px 20px -8px rgb(15 23 42 / 0.25);
    }
    .center { text-align: center; }
    .strong { font-weight: 700; }
    .small { font-size: 0.72rem; color: #444; }
    hr { border: 0; border-top: 1px dashed #999; margin: 0.5rem 0; }
    .item { margin-bottom: 0.35rem; }
    .row { display: flex; justify-content: space-between; gap: 0.5rem; }
    .indent { padding-left: 0.6rem; }
    .total { margin-top: 0.2rem; font-size: 0.95rem; font-weight: 700; }
    em { font-style: normal; font-size: 0.68rem; margin-left: 0.3rem; color: #555; }
    .stamp {
      position: absolute;
      top: 40%;
      left: 50%;
      transform: translate(-50%, -50%) rotate(-18deg);
      padding: 0.2rem 1rem;
      font-size: 1.6rem;
      font-weight: 800;
      letter-spacing: 0.2em;
      color: rgb(220 38 38 / 0.75);
      border: 3px solid rgb(220 38 38 / 0.75);
      border-radius: 6px;
      pointer-events: none;
    }
  `,
})
export class Receipt {
  readonly sale = input.required<SaleDetail>();
  private readonly auth = inject(AuthService);

  protected readonly method = METHOD;
  protected readonly pharmacy = computed(() => this.auth.pharmacyName() ?? 'Apotek');
  protected readonly top = computed(() => this.sale().items.filter((i) => i.parentItemId == null));

  protected components(parent: SaleItemDetail): SaleItemDetail[] {
    return this.sale().items.filter((i) => i.parentItemId === parent.id);
  }
}
