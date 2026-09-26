import { Component, OnInit, computed, inject, input, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { ConfirmationService } from 'primeng/api';
import { AutoCompleteModule } from 'primeng/autocomplete';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';
import { TooltipModule } from 'primeng/tooltip';

import type { Category } from '../../bindings/Category';
import type { NamedItem } from '../../bindings/NamedItem';
import type { OpnameDetail as Detail } from '../../bindings/OpnameDetail';
import type { OpnameItem } from '../../bindings/OpnameItem';
import type { OpnameItemInput } from '../../bindings/OpnameItemInput';
import type { ProductListRow } from '../../bindings/ProductListRow';
import { inventoryApi } from '../../core/api/inventory.api';
import { masterApi } from '../../core/api/master.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { confirmDelete } from '../../shared/confirm';
import { CostX100Pipe, DateTimePipe, RupiahPipe } from '../../shared/format';
import { CountSheetPrint } from './count-sheet-print';
import { OPNAME_TYPES, expiryState, formatDate, isoDate, opnameStatusInfo, toX100 } from './stock-format';

/** Form batch baru (stok awal atau batch yang ditemukan saat opname berkala). */
interface BatchForm {
  id: number | null;
  product: ProductListRow | null;
  batchNumber: string;
  expiryDate: string;
  /** HPP per satuan dasar dalam rupiah (boleh desimal). */
  cost: number | null;
  /** HPP lama sudah ada tetapi tidak terlihat (user tanpa VIEW_COST). */
  hasHiddenCost: boolean;
  qty: number | null;
  note: string;
}

/** Detail satu opname: isi hitung fisik per batch, ajukan, lalu setujui. */
@Component({
  selector: 'app-opname-detail',
  imports: [
    FormsModule,
    RouterLink,
    AutoCompleteModule,
    ButtonModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputNumberModule,
    InputTextModule,
    SelectModule,
    TableModule,
    TagModule,
    ToggleSwitchModule,
    TooltipModule,
    CostX100Pipe,
    DateTimePipe,
    RupiahPipe,
  ],
  templateUrl: './opname-detail.html',
  styleUrl: './opname-detail.scss',
})
export class OpnameDetail implements OnInit {
  /** Dari parameter rute `:id`. */
  readonly id = input.required<string>();

  private readonly notify = inject(Notify);
  private readonly confirm = inject(ConfirmationService);
  private readonly sheet = inject(CountSheetPrint);
  private readonly auth = inject(AuthService);
  protected readonly canApprove = this.auth.can('STOCK_COUNT_APPROVE');
  protected readonly viewCost = this.auth.can('VIEW_COST');

  protected readonly types = OPNAME_TYPES;
  protected readonly statusInfo = opnameStatusInfo;
  protected readonly formatDate = formatDate;
  protected readonly today = isoDate(new Date());

  protected readonly detail = signal<Detail | null>(null);
  protected readonly busy = signal(false);
  protected readonly isDraft = computed(() => this.detail()?.header.status === 'DRAFT');
  protected readonly isPeriodic = computed(() => this.detail()?.header.opnameType === 'PERIODIC');

  protected readonly filterText = signal('');
  protected readonly uncountedOnly = signal(false);
  protected readonly items = computed(() => {
    const q = this.filterText().trim().toLowerCase();
    return (this.detail()?.items ?? []).filter(
      (i) =>
        (!this.uncountedOnly() || i.physicalQtyBase == null) &&
        (!q ||
          i.productName.toLowerCase().includes(q) ||
          i.productCode.toLowerCase().includes(q) ||
          i.batchNumber.toLowerCase().includes(q)),
    );
  });

  // Dialog batch baru
  protected form: BatchForm | null = null;
  protected readonly suggestions = signal<ProductListRow[]>([]);
  protected readonly saving = signal(false);

  // Dialog isi dari batch
  protected filling: { productId: number | null; product: ProductListRow | null; rackId: number | null; categoryId: number | null } | null = null;
  protected readonly racks = signal<NamedItem[]>([]);
  protected readonly categories = signal<Category[]>([]);

  // Dialog batal
  protected cancelling: { reason: string } | null = null;

  ngOnInit(): void {
    this.load();
  }

  protected async load(): Promise<void> {
    try {
      this.detail.set(await inventoryApi.opnameGet(Number(this.id())));
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected diff(i: OpnameItem): number | null {
    return i.physicalQtyBase == null ? null : i.physicalQtyBase - i.systemQtyBase;
  }

  protected expiry(date: string) {
    return expiryState(date, this.today);
  }

  // ─── Hitung fisik langsung di tabel ───

  protected async savePhysical(item: OpnameItem, value: number | null): Promise<void> {
    if (value === item.physicalQtyBase) return;
    await this.saveItem(this.itemInput(item, { physicalQtyBase: value }));
  }

  protected async saveNote(item: OpnameItem, note: string): Promise<void> {
    if ((note.trim() || null) === item.note) return;
    await this.saveItem(this.itemInput(item, { note: note.trim() || null }));
  }

  /** Baris yang ada sebagai input simpan; HPP dikirim null agar HPP lama dipertahankan. */
  private itemInput(item: OpnameItem, patch: Partial<OpnameItemInput>): OpnameItemInput {
    return {
      opnameId: this.detail()!.header.id,
      id: item.id,
      productId: item.productId,
      batchId: item.batchId,
      batchNumber: item.batchId == null ? item.batchNumber : null,
      expiryDate: item.batchId == null ? item.expiryDate : null,
      unitCostX100: null,
      physicalQtyBase: item.physicalQtyBase,
      note: item.note,
      ...patch,
    };
  }

  private async saveItem(input: OpnameItemInput): Promise<boolean> {
    try {
      const r = await inventoryApi.opnameItemSave(input);
      this.detail.set(r.opname);
      this.notify.warnings(r.warnings);
      return true;
    } catch (e) {
      this.notify.error(e);
      await this.load();
      return false;
    }
  }

  /** Isi stok fisik = stok sistem (hasil hitung cocok, tanpa selisih). */
  protected async fillSameAsSystem(item: OpnameItem): Promise<void> {
    await this.savePhysical(item, item.systemQtyBase);
  }

  protected confirmDeleteItem(item: OpnameItem): void {
    confirmDelete(this.confirm, {
      header: 'Hapus baris?',
      message: 'Baris ini dihapus dari opname.',
      item: { name: `${item.productName} · ${item.batchNumber}`, code: item.productCode },
      accept: async () => {
        try {
          this.detail.set(await inventoryApi.opnameItemDelete(this.detail()!.header.id, item.id));
        } catch (e) {
          this.notify.error(e);
        }
      },
    });
  }

  // ─── Batch baru ───

  protected newBatch(): void {
    this.form = { id: null, product: this.form?.product ?? null, batchNumber: '', expiryDate: '', cost: null, hasHiddenCost: false, qty: null, note: '' };
  }

  protected editBatch(item: OpnameItem): void {
    this.form = {
      id: item.id,
      product: { id: item.productId, code: item.productCode, name: item.productName, baseUnitName: item.baseUnitName } as ProductListRow,
      batchNumber: item.batchNumber,
      expiryDate: item.expiryDate,
      cost: item.unitCostX100 == null ? null : item.unitCostX100 / 100,
      hasHiddenCost: item.hasCost && item.unitCostX100 == null,
      qty: item.physicalQtyBase,
      note: item.note ?? '',
    };
  }

  protected async searchProduct(query: string): Promise<void> {
    try {
      const r = await masterApi.productList({ q: query.trim() || null, categoryId: null, drugClass: null, includeInactive: false, offset: 0, limit: 20 });
      this.suggestions.set(r.rows);
    } catch (e) {
      this.notify.error(e);
    }
  }

  /** Simpan batch baru; `again` = kosongkan form untuk input berikutnya (obat tetap). */
  protected async saveBatch(again: boolean): Promise<void> {
    const f = this.form;
    if (!f) return;
    if (!f.product) {
      this.notify.warnings(['Pilih obat terlebih dahulu']);
      return;
    }
    this.saving.set(true);
    const ok = await this.saveItem({
      opnameId: this.detail()!.header.id,
      id: f.id,
      productId: f.product.id,
      batchId: null,
      batchNumber: f.batchNumber.trim() || null,
      expiryDate: f.expiryDate || null,
      unitCostX100: toX100(f.cost),
      physicalQtyBase: f.qty,
      note: f.note.trim() || null,
    });
    this.saving.set(false);
    if (!ok) return;
    this.notify.success(`Batch ${f.batchNumber.trim()} disimpan`);
    if (again && !f.id) {
      this.newBatch();
    } else {
      this.form = null;
    }
  }

  // ─── Isi dari batch yang ada (opname berkala) ───

  protected openFill(): void {
    if (!this.racks().length) {
      masterApi.rackList().then((r) => this.racks.set(r.filter((x) => x.isActive)), (e) => this.notify.error(e));
      masterApi.categoryList().then((c) => this.categories.set(c.filter((x) => x.isActive)), (e) => this.notify.error(e));
    }
    this.filling = { productId: null, product: null, rackId: null, categoryId: null };
  }

  protected async fill(): Promise<void> {
    const f = this.filling;
    if (!f) return;
    this.busy.set(true);
    try {
      const before = this.detail()?.items.length ?? 0;
      const r = await inventoryApi.opnameFill({
        opnameId: this.detail()!.header.id,
        productId: f.product?.id ?? null,
        rackId: f.rackId,
        categoryId: f.categoryId,
      });
      this.detail.set(r.opname);
      this.notify.warnings(r.warnings);
      const added = r.opname.items.length - before;
      if (added > 0) this.notify.success(`${added} batch ditambahkan`);
      this.filling = null;
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.busy.set(false);
    }
  }

  // ─── Status ───

  private async run(action: () => Promise<Detail>, message: string): Promise<void> {
    this.busy.set(true);
    try {
      this.detail.set(await action());
      this.notify.success(message);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.busy.set(false);
    }
  }

  protected submit(): void {
    const id = this.detail()!.header.id;
    this.run(() => inventoryApi.opnameSubmit(id), 'Opname diajukan untuk disetujui');
  }

  protected reopen(): void {
    const id = this.detail()!.header.id;
    this.run(() => inventoryApi.opnameReopen(id), 'Opname dikembalikan ke draft');
  }

  protected confirmApprove(): void {
    const d = this.detail()!;
    const changed = d.items.filter((i) => this.diff(i) !== 0).length;
    this.confirm.confirm({
      header: 'Setujui opname?',
      message:
        d.header.opnameType === 'OPENING'
          ? `Stok awal ${d.items.length} batch akan dicatat di kartu stok. Opname yang disetujui tidak bisa diubah.`
          : `${changed} baris dengan selisih akan menjadi penyesuaian stok. Opname yang disetujui tidak bisa diubah.`,
      icon: 'pi pi-check-circle',
      acceptLabel: 'Setujui',
      rejectLabel: 'Batal',
      rejectButtonProps: { severity: 'secondary', outlined: true },
      accept: () => this.run(() => inventoryApi.opnameApprove(d.header.id), `Opname ${d.header.number} disetujui, stok diperbarui`),
    });
  }

  protected async cancel(): Promise<void> {
    const c = this.cancelling;
    if (!c) return;
    const id = this.detail()!.header.id;
    await this.run(() => inventoryApi.opnameCancel(id, c.reason.trim() || null), 'Opname dibatalkan');
    this.cancelling = null;
  }

  protected printSheet(): void {
    const d = this.detail();
    if (d) this.sheet.print(d);
  }
}
