import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { SelectButtonModule } from 'primeng/selectbutton';
import { TableModule } from 'primeng/table';
import { TooltipModule } from 'primeng/tooltip';

import type { DebtFilter } from '../../bindings/DebtFilter';
import type { DebtSummary } from '../../bindings/DebtSummary';
import type { PurchaseDetail } from '../../bindings/PurchaseDetail';
import type { Supplier } from '../../bindings/Supplier';
import type { SupplierDebtRow } from '../../bindings/SupplierDebtRow';
import type { SupplierPaymentMethod } from '../../bindings/SupplierPaymentMethod';
import type { SupplierPaymentRow } from '../../bindings/SupplierPaymentRow';
import { purchasingApi } from '../../core/api/purchasing.api';
import { Notify } from '../../core/ui/notify';
import { RupiahPipe } from '../../shared/format';
import { formatDate } from '../inventory/stock-format';
import { MASTER_PAGE_SIZES, PAGE_REPORT } from '../master/master-table';
import { DEBT_FILTERS, PAYMENT_METHODS, dueLabel, paymentMethodLabel } from './purchase-labels';

interface PaymentForm {
  paymentDate: string;
  amount: number | null;
  method: SupplierPaymentMethod;
  reference: string;
  note: string;
}

/** Tab Hutang Supplier: faktur kredit yang belum lunas dan pembayarannya (hak SUPPLIER_DEBT_MANAGE). */
@Component({
  selector: 'app-debt-list',
  imports: [
    FormsModule,
    RouterLink,
    ButtonModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputNumberModule,
    InputTextModule,
    SelectModule,
    SelectButtonModule,
    TableModule,
    TooltipModule,
    RupiahPipe,
  ],
  templateUrl: './debt-list.html',
  styleUrl: './debt-list.scss',
})
export class DebtList {
  private readonly notify = inject(Notify);

  protected readonly filters = DEBT_FILTERS;
  protected readonly methods = PAYMENT_METHODS;
  protected readonly methodLabel = paymentMethodLabel;
  protected readonly dueLabel = dueLabel;
  protected readonly formatDate = formatDate;
  protected readonly pageSizes = MASTER_PAGE_SIZES;
  protected readonly pageReport = PAGE_REPORT;
  protected pageSize = MASTER_PAGE_SIZES[0];

  protected readonly rows = signal<SupplierDebtRow[]>([]);
  protected readonly total = signal(0);
  protected readonly summary = signal<DebtSummary | null>(null);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);
  protected readonly suppliers = signal<Supplier[]>([]);
  protected today = '';

  protected q = '';
  protected filter: DebtFilter = 'OPEN';
  protected supplierId: number | null = null;
  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  // Dialog pembayaran
  protected readonly paying = signal<PurchaseDetail | null>(null);
  protected payment: PaymentForm | null = null;
  protected voidingPayment: { payment: SupplierPaymentRow; reason: string } | null = null;
  protected readonly busy = signal(false);

  constructor() {
    purchasingApi.supplierList().then((s) => this.suppliers.set(s), (e) => this.notify.error(e));
  }

  protected async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await purchasingApi.debtPage({
        q: this.q.trim() || null,
        supplierId: this.supplierId,
        filter: this.filter,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
      this.summary.set(page.summary);
      this.today = page.today;
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected pageChanged(first: number, rows: number): void {
    this.pageSize = rows;
    this.load(first);
  }

  protected searchChanged(): void {
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.load(0), 250);
  }

  protected setFilter(f: DebtFilter): void {
    this.filter = f;
    this.load(0);
  }

  // ─── Pembayaran ───

  protected async openPayment(row: SupplierDebtRow): Promise<void> {
    try {
      const d = await purchasingApi.get(row.purchaseId);
      this.paying.set(d);
      this.resetPaymentForm(d);
    } catch (e) {
      this.notify.error(e);
    }
  }

  private resetPaymentForm(d: PurchaseDetail): void {
    this.payment =
      (d.outstanding ?? 0) > 0
        ? { paymentDate: this.today, amount: d.outstanding, method: 'TRANSFER', reference: '', note: '' }
        : null;
  }

  protected async pay(): Promise<void> {
    const d = this.paying();
    const f = this.payment;
    if (!d || !f) return;
    this.busy.set(true);
    try {
      const updated = await purchasingApi.paymentCreate({
        purchaseId: d.id,
        paymentDate: f.paymentDate,
        amount: f.amount ?? 0,
        method: f.method,
        reference: f.reference.trim() || null,
        note: f.note.trim() || null,
      });
      this.notify.success(
        (updated.outstanding ?? 0) > 0 ? `Pembayaran ${d.invoiceNumber} dicatat` : `Faktur ${d.invoiceNumber} lunas`,
      );
      this.paying.set(updated);
      this.resetPaymentForm(updated);
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.busy.set(false);
    }
  }

  protected async voidPayment(): Promise<void> {
    const v = this.voidingPayment;
    if (!v) return;
    this.busy.set(true);
    try {
      const updated = await purchasingApi.paymentVoid(v.payment.id, v.reason);
      this.voidingPayment = null;
      this.notify.success(`Pembayaran ${v.payment.number} dibatalkan`);
      this.paying.set(updated);
      this.resetPaymentForm(updated);
      await this.load();
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.busy.set(false);
    }
  }
}
