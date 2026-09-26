import { Component, ElementRef, HostListener, computed, inject, signal, viewChild } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TagModule } from 'primeng/tag';
import { TooltipModule } from 'primeng/tooltip';

import type { PaymentInput } from '../../bindings/PaymentInput';
import type { PosPrescription } from '../../bindings/PosPrescription';
import type { PosProduct } from '../../bindings/PosProduct';
import type { PosState } from '../../bindings/PosState';
import type { PrescriptionQuote } from '../../bindings/PrescriptionQuote';
import type { SaleDetail } from '../../bindings/SaleDetail';
import { salesApi } from '../../core/api/sales.api';
import { ApiError, toApiError } from '../../core/api/tauri';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe, formatRupiah } from '../../shared/format';
import { drugClassInfo } from '../../shared/labels';
import {
  type CartLine,
  baseNeedByProduct,
  cartTotals,
  discountOverLimit,
  lineGross,
  lineTotal,
  maxQty,
  newClientRef,
  paymentPlan,
  priceFor,
  remainingBase,
  unitOf,
} from './pos-cart';
import { Receipt } from './receipt';
import { SalesHistory } from './sales-history';

type NonCashMethod = 'QRIS' | 'DEBIT';

interface PinRequest {
  title: string;
  message: string;
  resolve: (pin: string | null) => void;
}

/**
 * Layar kasir. Semua keputusan (harga, stok FEFO, otorisasi) diambil Rust dalam satu transaksi;
 * layar ini menyusun keranjang, meminta PIN bila perlu, dan mengirim checkout satu kali dengan
 * kunci idempotensi (`clientRef`) yang sama sampai berhasil.
 */
@Component({
  selector: 'app-pos-page',
  imports: [
    FormsModule,
    ButtonModule,
    DialogModule,
    InputNumberModule,
    InputTextModule,
    SelectModule,
    TagModule,
    TooltipModule,
    RupiahPipe,
    DateTimePipe,
    Receipt,
    SalesHistory,
  ],
  templateUrl: './pos-page.html',
  styleUrl: './pos-page.scss',
})
export class PosPage {
  protected readonly auth = inject(AuthService);
  private readonly notify = inject(Notify);

  private readonly searchInput = viewChild<ElementRef<HTMLInputElement>>('searchInput');
  private readonly tenderedInput = viewChild<ElementRef<HTMLElement>>('tenderedInput');

  protected readonly drugClass = drugClassInfo;
  protected readonly unitOf = unitOf;
  protected readonly lineGross = lineGross;
  protected readonly lineTotal = lineTotal;
  protected readonly priceFor = priceFor;

  protected readonly state = signal<PosState | null>(null);
  protected readonly loading = signal(true);

  // ─── Pencarian ────────────────────────────────────────────────────────────
  protected q = '';
  protected readonly results = signal<PosProduct[]>([]);
  protected readonly activeResult = signal(0);
  protected readonly searching = signal(false);
  private searchTimer: ReturnType<typeof setTimeout> | undefined;
  private searchSeq = 0;

  // ─── Keranjang ────────────────────────────────────────────────────────────
  protected readonly lines = signal<CartLine[]>([]);
  private nextKey = 1;
  /** Resep yang sedang dibayar (menggantikan keranjang bebas). */
  protected readonly rx = signal<PrescriptionQuote | null>(null);

  // ─── Pembayaran ───────────────────────────────────────────────────────────
  protected nonCashMethod: NonCashMethod = 'QRIS';
  protected readonly nonCash = signal<number | null>(null);
  protected readonly tendered = signal<number | null>(null);
  protected reference = '';
  protected readonly paying = signal(false);
  /** Kunci idempotensi checkout ini; diganti hanya setelah berhasil atau keranjang dikosongkan. */
  private clientRef = newClientRef();

  protected readonly nonCashMethods = [
    { value: 'QRIS', label: 'QRIS' },
    { value: 'DEBIT', label: 'Debit' },
  ];

  // ─── Dialog ───────────────────────────────────────────────────────────────
  protected readonly receipt = signal<SaleDetail | null>(null);
  protected readonly historyOpen = signal(false);
  protected readonly pin = signal<PinRequest | null>(null);
  protected pinValue = '';
  protected openCash: number | null = null;
  protected readonly closeOpen = signal(false);
  protected closeCounted: number | null = null;
  protected closeNote = '';
  protected readonly rxListOpen = signal(false);
  protected readonly rxList = signal<PosPrescription[]>([]);

  protected readonly shift = computed(() => this.state()?.shift ?? null);
  protected readonly roundingUnit = computed(() => this.state()?.totalRounding ?? 0);

  protected readonly totals = computed(() => {
    const rx = this.rx();
    if (rx) {
      return { subtotal: rx.subtotal, discount: 0, rounding: rx.rounding, total: rx.grandTotal };
    }
    return cartTotals(this.lines(), this.roundingUnit());
  });

  protected readonly plan = computed(() => paymentPlan(this.totals().total, this.nonCash(), this.tendered()));

  protected readonly itemCount = computed(() => (this.rx() ? this.rx()!.lines.length : this.lines().length));

  /** Obat yang kebutuhannya melebihi stok terakhir yang terlihat (peringatan, Rust tetap memutuskan). */
  protected readonly shortStock = computed(() => {
    const need = baseNeedByProduct(this.lines());
    const short = new Set<number>();
    for (const l of this.lines()) {
      if ((need.get(l.product.productId) ?? 0) > l.product.sellableBase) short.add(l.product.productId);
    }
    return short;
  });

  protected readonly needsHardPin = computed(
    () => !this.rx() && !this.auth.can('SELL_HARD_DRUG') && this.lines().some((l) => l.product.drugClass === 'HARD'),
  );

  protected readonly needsDiscountPin = computed(() => {
    const max = this.state()?.maxDiscountBp ?? 0;
    return !this.rx() && !this.auth.can('DISCOUNT_OVERRIDE') && this.lines().some((l) => discountOverLimit(l, max));
  });

  protected readonly canPay = computed(
    () =>
      !!this.shift() &&
      !this.paying() &&
      this.itemCount() > 0 &&
      this.plan().short === 0 &&
      !(this.rx()?.problems.length ?? 0),
  );

  constructor() {
    this.reload();
  }

  protected async reload(): Promise<void> {
    this.loading.set(true);
    try {
      this.state.set(await salesApi.state());
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
      this.focusSearch();
    }
  }

  // ─── Tombol pintas ────────────────────────────────────────────────────────

  @HostListener('window:keydown', ['$event'])
  protected onKey(e: KeyboardEvent): void {
    if (this.receipt()) {
      if (e.key === 'Enter' || e.key === 'Escape') {
        e.preventDefault();
        this.newTransaction();
      }
      return;
    }
    if (this.pin() || this.closeOpen() || this.historyOpen() || this.rxListOpen()) return;
    switch (e.key) {
      case 'F3':
        e.preventDefault();
        this.focusSearch();
        break;
      case 'F4':
        e.preventDefault();
        this.tenderedInput()?.nativeElement.querySelector('input')?.focus();
        break;
      case 'F6':
        e.preventDefault();
        this.openRxList();
        break;
      case 'F7':
        e.preventDefault();
        this.historyOpen.set(true);
        break;
      case 'F9':
        e.preventDefault();
        this.pay();
        break;
    }
  }

  protected defaultUnit(p: PosProduct) {
    return p.units.find((u) => u.isDefault) ?? p.units[0];
  }

  private focusSearch(): void {
    setTimeout(() => this.searchInput()?.nativeElement.focus());
  }

  // ─── Pencarian & keranjang ────────────────────────────────────────────────

  protected searchChanged(): void {
    clearTimeout(this.searchTimer);
    const q = this.q.trim();
    if (!q) {
      this.results.set([]);
      return;
    }
    this.searchTimer = setTimeout(() => this.runSearch(q), 180);
  }

  private async runSearch(q: string): Promise<PosProduct[]> {
    const seq = ++this.searchSeq;
    this.searching.set(true);
    try {
      const found = await salesApi.search(q);
      // Abaikan hasil pencarian lama yang datang terlambat.
      if (seq === this.searchSeq) {
        this.results.set(found);
        this.activeResult.set(0);
      }
      return found;
    } catch (e) {
      this.notify.error(e);
      return [];
    } finally {
      if (seq === this.searchSeq) this.searching.set(false);
    }
  }

  protected async searchKey(e: KeyboardEvent): Promise<void> {
    const n = this.results().length;
    if (e.key === 'ArrowDown' && n) {
      e.preventDefault();
      this.activeResult.set((this.activeResult() + 1) % n);
    } else if (e.key === 'ArrowUp' && n) {
      e.preventDefault();
      this.activeResult.set((this.activeResult() - 1 + n) % n);
    } else if (e.key === 'Escape') {
      this.q = '';
      this.results.set([]);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const q = this.q.trim();
      if (!q) return;
      // Scanner barcode mengetik lalu Enter lebih cepat dari jeda pencarian: cari langsung.
      clearTimeout(this.searchTimer);
      const found = this.results().length && !this.searching() ? this.results() : await this.runSearch(q);
      const pick = found[this.activeResult()] ?? found[0];
      if (pick) this.add(pick, pick.matchedUnitId);
      else this.notify.warnings([`Obat "${q}" tidak ditemukan`]);
    }
  }

  protected add(p: PosProduct, unitId: number | null = null): void {
    if (this.rx()) {
      this.notify.warnings(['Sedang membayar resep. Batalkan dulu untuk penjualan bebas.']);
      return;
    }
    if (p.drugClass === 'PSYCHOTROPIC' || p.drugClass === 'NARCOTIC') {
      this.notify.warnings([`${p.name} (${drugClassInfo(p.drugClass).label}) hanya bisa dijual lewat resep`]);
      return;
    }
    if (!p.units.length) {
      this.notify.warnings([`${p.name} belum punya satuan jual aktif`]);
      return;
    }
    if (p.sellableBase <= 0) {
      // Stok bisa dijual = batch belum ED & tidak terkunci; stok ED/terkunci tidak dihitung.
      this.notify.warnings([`Stok ${p.name} habis, tidak bisa ditambahkan ke keranjang`]);
      this.q = '';
      this.results.set([]);
      this.focusSearch();
      return;
    }
    const unit = unitId ?? p.units.find((u) => u.isDefault)?.productUnitId ?? p.units[0].productUnitId;
    const existing = this.lines().find((l) => l.product.productId === p.productId && l.productUnitId === unit);
    if (existing) {
      // Scan ulang obat + satuan yang sama menambah qty baris yang ada (tier ikut dihitung ulang).
      this.updateLine(existing.key, { qty: existing.qty + 1, product: p });
    } else {
      const line: CartLine = { key: this.nextKey++, product: p, productUnitId: unit, qty: 1, discount: 0 };
      if (maxQty(this.lines(), line) < 1) {
        this.notify.warnings([this.stockMessage(this.lines(), line)]);
      } else {
        this.lines.update((ls) => [...ls, line]);
      }
    }
    this.q = '';
    this.results.set([]);
    this.focusSearch();
  }

  /**
   * Ubah baris. Qty dibatasi sampai stok bisa-jual (dikurangi baris lain obat yang sama);
   * ganti satuan yang tidak muat di sisa stok ditolak.
   */
  protected updateLine(key: number, patch: Partial<CartLine>): void {
    const ls = this.lines();
    const current = ls.find((l) => l.key === key);
    if (!current) return;
    const next = { ...current, ...patch };
    const duplicate = ls.some(
      (l) => l.key !== key && l.product.productId === next.product.productId && l.productUnitId === next.productUnitId,
    );
    if (duplicate) {
      this.notify.warnings([`${next.product.name} satuan ${unitOf(next).unitName} sudah ada di keranjang; ubah jumlah di baris itu`]);
      this.lines.set(ls.map((l) => (l.key === key ? { ...current } : l)));
      return;
    }
    next.qty = Math.max(1, Math.min(Math.round(next.qty || 1), 100_000));
    const max = maxQty(ls, next);
    if (max < 1) {
      this.notify.warnings([this.stockMessage(ls, next)]);
      // Salinan baru agar input satuan/qty di layar kembali ke nilai lama.
      this.lines.set(ls.map((l) => (l.key === key ? { ...current } : l)));
      return;
    }
    if (next.qty > max) {
      const u = unitOf(next);
      this.notify.warnings([`Jumlah ${next.product.name} dibatasi ${max} ${u.unitName} sesuai stok tersedia`]);
      next.qty = max;
    }
    next.discount = Math.max(0, Math.min(Math.round(next.discount || 0), lineGross(next)));
    this.lines.set(ls.map((l) => (l.key === key ? next : l)));
  }

  /** Qty maksimal di input jumlah (minimal 1 agar baris yang sudah ada tetap bisa ditampilkan). */
  protected maxQtyOf(line: CartLine): number {
    return Math.max(1, Math.min(maxQty(this.lines(), line), 100_000));
  }

  /**
   * Pilihan satuan per baris. Dinonaktifkan: satuan yang tidak muat di sisa stok, dan satuan yang
   * sudah dipakai baris lain obat yang sama (agar satu obat + satuan hanya satu baris).
   */
  protected readonly unitChoices = computed(() => {
    const ls = this.lines();
    return new Map(
      ls.map((l) => [
        l.key,
        l.product.units.map((u) => ({
          ...u,
          disabled:
            u.productUnitId !== l.productUnitId &&
            (maxQty(ls, l, u.productUnitId) < 1 ||
              ls.some((o) => o.key !== l.key && o.product.productId === l.product.productId && o.productUnitId === u.productUnitId)),
        })),
      ]),
    );
  });

  private stockMessage(ls: CartLine[], line: CartLine): string {
    const left = remainingBase(ls, line);
    const u = unitOf(line);
    const base = line.product.baseUnitName;
    const inCart = line.product.sellableBase - left;
    return left <= 0
      ? `Stok ${line.product.name} (${line.product.sellableBase} ${base}) sudah semuanya ada di keranjang`
      : `Sisa stok ${line.product.name} ${left} ${base}${inCart ? ' (setelah yang di keranjang)' : ''}, tidak cukup untuk 1 ${u.unitName} (${u.conversion} ${base}). Pilih satuan lebih kecil.`;
  }

  protected removeLine(key: number): void {
    this.lines.update((ls) => ls.filter((l) => l.key !== key));
    if (!this.lines().length) this.resetPayment();
    this.focusSearch();
  }

  protected clearCart(): void {
    this.lines.set([]);
    this.rx.set(null);
    this.resetPayment();
    this.clientRef = newClientRef();
    this.focusSearch();
  }

  private resetPayment(): void {
    this.nonCash.set(null);
    this.tendered.set(null);
    this.reference = '';
  }

  // ─── Resep ────────────────────────────────────────────────────────────────

  protected async openRxList(): Promise<void> {
    if (this.lines().length) {
      this.notify.warnings(['Selesaikan atau kosongkan keranjang dulu sebelum membayar resep']);
      return;
    }
    try {
      this.rxList.set(await salesApi.prescriptions());
      this.rxListOpen.set(true);
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected async pickRx(p: PosPrescription): Promise<void> {
    try {
      const quote = await salesApi.prescriptionQuote(p.id);
      this.rx.set(quote);
      this.resetPayment();
      this.clientRef = newClientRef();
      this.rxListOpen.set(false);
    } catch (e) {
      this.notify.error(e);
    }
  }

  // ─── Bayar ────────────────────────────────────────────────────────────────

  private askPin(title: string, message: string): Promise<string | null> {
    this.pinValue = '';
    return new Promise((resolve) => this.pin.set({ title, message, resolve }));
  }

  protected submitPin(ok: boolean): void {
    const req = this.pin();
    this.pin.set(null);
    req?.resolve(ok && this.pinValue.trim() ? this.pinValue.trim() : null);
    this.pinValue = '';
  }

  protected async pay(): Promise<void> {
    if (!this.canPay()) {
      if (this.plan().short > 0) this.notify.warnings([`Uang tunai kurang ${formatRupiah(this.plan().short)}`]);
      return;
    }
    // Kunci layar dulu agar Enter/F9 berulang tidak memicu checkout kedua selama menunggu PIN.
    this.paying.set(true);
    try {
      let hardPin: string | null = null;
      let discountPin: string | null = null;
      if (this.needsHardPin()) {
        const names = this.lines()
          .filter((l) => l.product.drugClass === 'HARD')
          .map((l) => l.product.name)
          .join(', ');
        hardPin = await this.askPin('Otorisasi obat keras', `${names} — masukkan PIN apoteker.`);
        if (!hardPin) return;
      }
      if (this.needsDiscountPin()) {
        discountPin = await this.askPin('Otorisasi diskon', 'Diskon melebihi batas — masukkan PIN Pemilik/Apoteker.');
        if (!discountPin) return;
      }
      const t = this.totals();
      const plan = this.plan();
      const payments: PaymentInput[] = [];
      if (plan.nonCash > 0) {
        payments.push({ method: this.nonCashMethod, amount: plan.nonCash, tendered: null, reference: this.reference.trim() || null });
      }
      if (plan.cashPart > 0) {
        payments.push({ method: 'CASH', amount: plan.cashPart, tendered: plan.tendered, reference: null });
      }
      const rx = this.rx();
      const sale = await salesApi.create({
        clientRef: this.clientRef,
        prescriptionId: rx?.id ?? null,
        items: rx
          ? []
          : this.lines().map((l) => ({ productUnitId: l.productUnitId, qty: l.qty, discountAmount: l.discount })),
        payments,
        expectedTotal: t.total,
        hardDrugPin: hardPin,
        discountPin,
      });
      this.receipt.set(sale);
      this.lines.set([]);
      this.rx.set(null);
      this.resetPayment();
      this.clientRef = newClientRef();
      this.refreshState();
    } catch (e) {
      const err = toApiError(e);
      this.notify.error(err);
      await this.recover(err);
    } finally {
      this.paying.set(false);
    }
  }

  /** Setelah checkout ditolak: segarkan harga/stok agar layar sesuai data terbaru. */
  private async recover(err: ApiError): Promise<void> {
    if (err.code !== 'CONFLICT') return;
    await this.refreshState();
    const rx = this.rx();
    if (rx) {
      try {
        this.rx.set(await salesApi.prescriptionQuote(rx.id));
      } catch {
        this.rx.set(null);
      }
      return;
    }
    await this.refreshProducts();
  }

  /** Ambil ulang harga, tier, dan stok setiap obat di keranjang. */
  private async refreshProducts(): Promise<void> {
    const fresh = new Map<number, PosProduct>();
    for (const code of new Set(this.lines().map((l) => l.product.code))) {
      const found = await salesApi.search(code).catch(() => []);
      for (const p of found) fresh.set(p.productId, p);
    }
    this.lines.update((ls) =>
      ls
        .filter((l) => fresh.has(l.product.productId))
        .map((l) => {
          const p = fresh.get(l.product.productId)!;
          const unitOk = p.units.some((u) => u.productUnitId === l.productUnitId);
          return { ...l, product: p, productUnitId: unitOk ? l.productUnitId : p.units[0].productUnitId };
        }),
    );
  }

  private async refreshState(): Promise<void> {
    try {
      this.state.set(await salesApi.state());
    } catch {
      /* ditampilkan saat reload berikutnya */
    }
  }

  protected newTransaction(): void {
    this.receipt.set(null);
    this.focusSearch();
  }

  // ─── Shift ────────────────────────────────────────────────────────────────

  protected async openShift(): Promise<void> {
    try {
      await salesApi.shiftOpen({ openingCash: this.openCash ?? 0, note: null });
      this.notify.success('Shift dibuka');
      this.openCash = null;
      await this.reload();
    } catch (e) {
      this.notify.error(e);
      await this.refreshState();
    }
  }

  protected startClose(): void {
    if (this.lines().length || this.rx()) {
      this.notify.warnings(['Selesaikan atau kosongkan keranjang sebelum menutup shift']);
      return;
    }
    this.closeCounted = null;
    this.closeNote = '';
    this.refreshState();
    this.closeOpen.set(true);
  }

  protected async closeShift(): Promise<void> {
    const s = this.shift();
    if (!s || this.closeCounted == null) return;
    try {
      const closed = await salesApi.shiftClose({ shiftId: s.id, countedCash: this.closeCounted, note: this.closeNote.trim() || null });
      const diff = closed.figures?.difference ?? 0;
      this.notify.success(
        diff === 0 ? 'Shift ditutup, uang di laci sesuai' : `Shift ditutup, selisih ${formatRupiah(diff)}`,
      );
      this.closeOpen.set(false);
      await this.reload();
    } catch (e) {
      this.notify.error(e);
      await this.refreshState();
    }
  }

  protected closeDifference(): number | null {
    const f = this.shift()?.figures;
    return f && this.closeCounted != null ? this.closeCounted - f.expectedCash : null;
  }

  protected onHistoryChanged(): void {
    this.refreshState();
  }
}
