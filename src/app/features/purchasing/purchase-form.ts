import { Component, OnInit, inject, input, signal, viewChild } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router, RouterLink } from '@angular/router';
import { ConfirmationService } from 'primeng/api';
import { AutoComplete, AutoCompleteModule, type AutoCompleteCompleteEvent, type AutoCompleteSelectEvent } from 'primeng/autocomplete';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { SelectButtonModule } from 'primeng/selectbutton';
import { TagModule } from 'primeng/tag';
import { TooltipModule } from 'primeng/tooltip';

import type { DrugClass } from '../../bindings/DrugClass';
import type { PurchaseDefaults } from '../../bindings/PurchaseDefaults';
import type { PurchaseDetail } from '../../bindings/PurchaseDetail';
import type { PurchaseInput } from '../../bindings/PurchaseInput';
import type { PurchaseItemDetail } from '../../bindings/PurchaseItemDetail';
import type { PurchasePaymentType } from '../../bindings/PurchasePaymentType';
import type { PurchaseProduct } from '../../bindings/PurchaseProduct';
import type { PurchaseUnit } from '../../bindings/PurchaseUnit';
import type { Supplier } from '../../bindings/Supplier';
import type { TaxMode } from '../../bindings/TaxMode';
import { purchasingApi } from '../../core/api/purchasing.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { CostX100Pipe, DateTimePipe, RupiahPipe, bpToPercent, percentToBp } from '../../shared/format';
import { drugClassInfo } from '../../shared/labels';
import { expiryState, formatDate } from '../inventory/stock-format';
import { SupplierDialog } from '../master/suppliers/supplier-dialog';
import { computeInvoice, lineTotal, type CalcTotals } from './purchase-calc';
import { PAYMENT_TYPES, TAX_MODES, addDays, paymentMethodLabel, purchaseStatusInfo } from './purchase-labels';

/** Satu baris faktur di form. Diskon dalam persen (boleh desimal). */
interface FormLine {
  key: number;
  productId: number;
  code: string;
  name: string;
  drugClass: DrugClass;
  baseUnitName: string;
  units: PurchaseUnit[];
  productUnitId: number;
  qty: number | null;
  bonusQty: number | null;
  unitPrice: number | null;
  disc1: number | null;
  disc2: number | null;
  batchNumber: string;
  expiryDate: string;
  /** HPP acuan obat (VIEW_COST), pembanding harga naik. */
  lastCostX100: number | null;
}

interface Header {
  supplierId: number | null;
  invoiceNumber: string;
  invoiceDate: string;
  receivedDate: string;
  dueDate: string;
  paymentType: PurchasePaymentType;
  taxMode: TaxMode;
  /** Persen. */
  taxRate: number | null;
  extraDiscount: number | null;
  note: string;
}

/** Faktur pembelian: input draft, posting (stok bertambah), dan batal. */
@Component({
  selector: 'app-purchase-form',
  imports: [
    FormsModule,
    RouterLink,
    AutoCompleteModule,
    ButtonModule,
    CheckboxModule,
    DialogModule,
    InputNumberModule,
    InputTextModule,
    SelectModule,
    SelectButtonModule,
    TagModule,
    TooltipModule,
    CostX100Pipe,
    DateTimePipe,
    RupiahPipe,
    SupplierDialog,
  ],
  templateUrl: './purchase-form.html',
  styleUrl: './purchase-form.scss',
  host: { '(document:keydown.control.s)': 'onSaveKey($event)' },
})
export class PurchaseForm implements OnInit {
  /** Dari parameter rute `:id`; `baru` = faktur baru. */
  readonly id = input.required<string>();

  private readonly notify = inject(Notify);
  private readonly router = inject(Router);
  private readonly confirm = inject(ConfirmationService);
  private readonly auth = inject(AuthService);
  protected readonly viewCost = this.auth.can('VIEW_COST');
  protected readonly canVoidPosted = this.auth.can('TRANSACTION_VOID');
  protected readonly canDebt = this.auth.can('SUPPLIER_DEBT_MANAGE');

  protected readonly paymentTypes = PAYMENT_TYPES;
  protected readonly taxModes = TAX_MODES;
  protected readonly statusInfo = purchaseStatusInfo;
  protected readonly formatDate = formatDate;
  protected readonly drugClassInfo = drugClassInfo;
  protected readonly methodLabel = paymentMethodLabel;
  protected readonly unitLabel = (u: PurchaseUnit) => (u.conversion > 1 ? `${u.unitName} (${u.conversion})` : u.unitName);

  protected readonly detail = signal<PurchaseDetail | null>(null);
  protected readonly defaults = signal<PurchaseDefaults | null>(null);
  protected readonly suppliers = signal<Supplier[]>([]);
  protected readonly loaded = signal(false);
  protected readonly busy = signal(false);

  protected header: Header = {
    supplierId: null,
    invoiceNumber: '',
    invoiceDate: '',
    receivedDate: '',
    dueDate: '',
    paymentType: 'CREDIT',
    taxMode: 'EXCLUDED',
    taxRate: 11,
    extraDiscount: null,
    note: '',
  };
  protected lines: FormLine[] = [];
  private nextKey = 1;
  /** Isi form terakhir yang tersimpan, untuk mendeteksi perubahan belum disimpan. */
  private savedJson = '';

  // Pencarian obat
  private readonly productSearch = viewChild(AutoComplete);
  protected search: PurchaseProduct | string | null = null;
  protected readonly suggestions = signal<PurchaseProduct[]>([]);

  // Dialog
  protected supplierDialog: { name: string } | null = null;
  protected posting: { updatePrices: boolean } | null = null;
  protected voiding: { reason: string } | null = null;

  protected get editable(): boolean {
    const d = this.detail();
    return !d || d.status === 'DRAFT';
  }

  async ngOnInit(): Promise<void> {
    try {
      const [defaults, suppliers] = await Promise.all([purchasingApi.defaults(), purchasingApi.supplierList()]);
      this.defaults.set(defaults);
      this.suppliers.set(suppliers);
      if (this.id() === 'baru') {
        this.header.invoiceDate = defaults.today;
        this.header.receivedDate = defaults.today;
        this.header.taxRate = bpToPercent(defaults.taxRateBp);
        this.savedJson = JSON.stringify(this.toInput());
      } else {
        this.apply(await purchasingApi.get(Number(this.id())));
      }
      this.loaded.set(true);
    } catch (e) {
      this.notify.error(e);
    }
  }

  /** Isi form dari detail faktur (setelah muat atau simpan). */
  private apply(d: PurchaseDetail): void {
    this.detail.set(d);
    this.header = {
      supplierId: d.supplierId,
      invoiceNumber: d.invoiceNumber,
      invoiceDate: d.invoiceDate,
      receivedDate: d.receivedDate,
      dueDate: d.dueDate ?? '',
      paymentType: d.paymentType,
      taxMode: d.taxMode,
      taxRate: bpToPercent(d.taxRateBp),
      extraDiscount: d.extraDiscount || null,
      note: d.note ?? '',
    };
    this.lines = d.items.map((i) => this.lineFromItem(i));
    this.savedJson = JSON.stringify(this.toInput());
  }

  private lineFromItem(i: PurchaseItemDetail): FormLine {
    return {
      key: this.nextKey++,
      productId: i.productId,
      code: i.productCode,
      name: i.productName,
      drugClass: i.drugClass,
      baseUnitName: i.baseUnitName,
      units: i.units.length ? i.units : [{ productUnitId: i.productUnitId, unitName: i.unitName, conversion: i.conversion }],
      productUnitId: i.productUnitId,
      qty: i.qty,
      bonusQty: i.bonusQty || null,
      unitPrice: i.unitPrice,
      disc1: bpToPercent(i.discount1Bp) || null,
      disc2: bpToPercent(i.discount2Bp) || null,
      batchNumber: i.batchNumber,
      expiryDate: i.expiryDate,
      lastCostX100: i.lastCostX100,
    };
  }

  protected toInput(): PurchaseInput {
    const h = this.header;
    return {
      id: this.detail()?.id ?? null,
      supplierId: h.supplierId ?? 0,
      invoiceNumber: h.invoiceNumber,
      invoiceDate: h.invoiceDate,
      receivedDate: h.receivedDate,
      dueDate: h.paymentType === 'CREDIT' ? h.dueDate || null : null,
      paymentType: h.paymentType,
      taxMode: h.taxMode,
      taxRateBp: h.taxMode === 'NONE' ? 0 : (percentToBp(h.taxRate) ?? 0),
      extraDiscount: h.extraDiscount ?? 0,
      note: h.note.trim() || null,
      items: this.lines.map((l) => ({
        productUnitId: l.productUnitId,
        qty: l.qty ?? 0,
        bonusQty: l.bonusQty ?? 0,
        unitPrice: l.unitPrice ?? 0,
        discount1Bp: percentToBp(l.disc1) ?? 0,
        discount2Bp: percentToBp(l.disc2) ?? 0,
        batchNumber: l.batchNumber,
        expiryDate: l.expiryDate,
      })),
    };
  }

  protected dirty(): boolean {
    return this.editable && JSON.stringify(this.toInput()) !== this.savedJson;
  }

  // ─── Perhitungan (pratinjau; angka final dari Rust) ───

  private conversion(l: FormLine): number {
    return l.units.find((u) => u.productUnitId === l.productUnitId)?.conversion ?? 1;
  }

  protected unitName(l: FormLine): string {
    return l.units.find((u) => u.productUnitId === l.productUnitId)?.unitName ?? '';
  }

  protected totals(): CalcTotals {
    const d = this.detail();
    const taxInCost = d && d.status !== 'DRAFT' ? d.taxInCost : !(this.defaults()?.isPkp ?? false);
    const input = this.toInput();
    return computeInvoice(
      this.lines.map((l, i) => ({
        qty: input.items[i].qty,
        bonusQty: input.items[i].bonusQty,
        conversion: this.conversion(l),
        unitPrice: input.items[i].unitPrice,
        discount1Bp: input.items[i].discount1Bp,
        discount2Bp: input.items[i].discount2Bp,
      })),
      input.extraDiscount,
      input.taxMode,
      input.taxRateBp,
      taxInCost,
    );
  }

  protected lineTotal(l: FormLine): number {
    return lineTotal({
      qty: l.qty ?? 0,
      unitPrice: l.unitPrice ?? 0,
      discount1Bp: percentToBp(l.disc1) ?? 0,
      discount2Bp: percentToBp(l.disc2) ?? 0,
    });
  }

  /** HPP baris: hasil posting (snapshot) atau pratinjau untuk draft. */
  protected lineCost(index: number, totals: CalcTotals): number | null {
    const d = this.detail();
    if (d && d.status !== 'DRAFT') return d.items[index]?.unitCostX100 ?? null;
    return totals.unitCostsX100[index] ?? null;
  }

  protected qtyBase(l: FormLine): number {
    return ((l.qty ?? 0) + (l.bonusQty ?? 0)) * this.conversion(l);
  }

  protected expiry(l: FormLine) {
    return l.expiryDate ? expiryState(l.expiryDate, this.header.receivedDate || this.defaults()?.today || '') : 'ok';
  }

  // ─── Header ───

  protected supplierOptions(): Supplier[] {
    const id = this.header.supplierId;
    return this.suppliers().filter((s) => s.isActive || s.id === id);
  }

  protected supplierChanged(): void {
    const s = this.suppliers().find((x) => x.id === this.header.supplierId);
    if (!s) return;
    if (!this.detail()) {
      this.header.paymentType = s.paymentTermDays > 0 ? 'CREDIT' : 'CASH';
    }
    this.updateDueDate();
  }

  /** Jatuh tempo = tanggal faktur + tempo supplier. */
  protected updateDueDate(): void {
    const h = this.header;
    const s = this.suppliers().find((x) => x.id === h.supplierId);
    if (h.paymentType === 'CREDIT' && h.invoiceDate && s) {
      h.dueDate = addDays(h.invoiceDate, s.paymentTermDays);
    }
  }

  protected async supplierSaved(s: Supplier): Promise<void> {
    this.supplierDialog = null;
    try {
      this.suppliers.set(await purchasingApi.supplierList());
      this.header.supplierId = s.id;
      this.supplierChanged();
    } catch (e) {
      this.notify.error(e);
    }
  }

  // ─── Baris ───

  protected async searchProducts(e: AutoCompleteCompleteEvent): Promise<void> {
    try {
      const rows = await purchasingApi.productSearch(e.query);
      // Barcode persis: langsung jadi baris dengan satuan yang di-scan.
      if (rows.length === 1 && rows[0].matchedUnitId != null) {
        this.addLine(rows[0]);
        return;
      }
      this.suggestions.set(rows);
    } catch (err) {
      this.notify.error(err);
    }
  }

  protected productSelected(e: AutoCompleteSelectEvent): void {
    this.addLine(e.value as PurchaseProduct);
  }

  private addLine(p: PurchaseProduct): void {
    if (!p.units.length) {
      this.notify.warnings([`${p.name} belum punya satuan aktif`]);
      return;
    }
    const line: FormLine = {
      key: this.nextKey++,
      productId: p.productId,
      code: p.code,
      name: p.name,
      drugClass: p.drugClass,
      baseUnitName: p.baseUnitName,
      units: p.units,
      // Satuan terbesar (box) paling umum di faktur PBF.
      productUnitId: p.matchedUnitId ?? p.units[0].productUnitId,
      qty: 1,
      bonusQty: null,
      unitPrice: null,
      disc1: null,
      disc2: null,
      batchNumber: '',
      expiryDate: '',
      lastCostX100: p.lastCostX100,
    };
    this.lines = [...this.lines, line];
    this.suggestions.set([]);
    // Kosongkan kotak pencarian (model tetap null sehingga binding ngModel tidak menulis ulang).
    setTimeout(() => this.productSearch()?.clear());
    // Setelah autocomplete selesai mengembalikan fokus ke dirinya sendiri.
    setTimeout(() => {
      const qty = document.getElementById(`qty-${line.key}`)?.querySelector('input');
      qty?.focus();
      qty?.select();
    }, 50);
  }

  protected removeLine(l: FormLine): void {
    this.lines = this.lines.filter((x) => x !== l);
  }

  /** Salin baris (obat sama, batch lain). */
  protected duplicateLine(l: FormLine): void {
    const copy = { ...l, key: this.nextKey++, batchNumber: '', expiryDate: '', qty: 1, bonusQty: null };
    const i = this.lines.indexOf(l);
    this.lines = [...this.lines.slice(0, i + 1), copy, ...this.lines.slice(i + 1)];
    setTimeout(() => document.getElementById(`batch-${copy.key}`)?.focus());
  }

  // ─── Simpan, posting, batal ───

  protected onSaveKey(e: Event): void {
    if (!this.editable) return;
    e.preventDefault();
    this.save();
  }

  /** Simpan draft. Mengembalikan true bila berhasil. */
  protected async save(silent = false): Promise<boolean> {
    if (!this.header.supplierId) {
      this.notify.warnings(['Pilih supplier terlebih dahulu']);
      return false;
    }
    this.busy.set(true);
    try {
      const isNew = !this.detail();
      const r = await purchasingApi.save(this.toInput());
      this.apply(r.purchase);
      this.notify.warnings(r.warnings);
      if (!silent) this.notify.success(`Faktur ${r.purchase.number} tersimpan sebagai draft`);
      if (isNew) {
        this.router.navigate(['/pembelian/faktur', r.purchase.id], { replaceUrl: true });
      }
      return true;
    } catch (e) {
      this.notify.error(e);
      return false;
    } finally {
      this.busy.set(false);
    }
  }

  protected openPost(): void {
    if (!this.lines.length) {
      this.notify.warnings(['Faktur belum berisi obat']);
      return;
    }
    this.posting = { updatePrices: true };
  }

  protected async post(): Promise<void> {
    const p = this.posting;
    if (!p) return;
    if ((this.dirty() || !this.detail()) && !(await this.save(true))) return;
    this.busy.set(true);
    try {
      const r = await purchasingApi.post({ id: this.detail()!.id, updatePrices: p.updatePrices });
      this.apply(r.purchase);
      this.posting = null;
      this.notify.success(`Faktur ${r.purchase.number} diposting, stok bertambah`);
      this.notify.warnings(r.warnings);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.busy.set(false);
    }
  }

  protected openVoid(): void {
    this.voiding = { reason: '' };
  }

  protected async voidPurchase(): Promise<void> {
    const v = this.voiding;
    const d = this.detail();
    if (!v || !d) return;
    this.busy.set(true);
    try {
      this.apply(await purchasingApi.void(d.id, v.reason.trim() || null));
      this.voiding = null;
      this.notify.success(`Faktur ${d.number} dibatalkan`);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.busy.set(false);
    }
  }

  protected discard(): void {
    if (!this.dirty()) {
      this.router.navigate(['/pembelian/faktur']);
      return;
    }
    this.confirm.confirm({
      header: 'Tinggalkan faktur?',
      message: 'Perubahan yang belum disimpan akan hilang.',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Tinggalkan',
      rejectLabel: 'Kembali',
      rejectButtonProps: { severity: 'secondary', outlined: true },
      accept: () => this.router.navigate(['/pembelian/faktur']),
    });
  }
}
