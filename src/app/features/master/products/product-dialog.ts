import { Component, computed, inject, input, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { Popover, PopoverModule } from 'primeng/popover';
import { RadioButtonModule } from 'primeng/radiobutton';
import { SelectModule } from 'primeng/select';
import { TabsModule } from 'primeng/tabs';
import { TagModule } from 'primeng/tag';
import { TooltipModule } from 'primeng/tooltip';

import type { Category } from '../../../bindings/Category';
import type { DrugClass } from '../../../bindings/DrugClass';
import type { NamedItem } from '../../../bindings/NamedItem';
import type { PriceMode } from '../../../bindings/PriceMode';
import type { ProductDetail } from '../../../bindings/ProductDetail';
import type { Unit } from '../../../bindings/Unit';
import { masterApi } from '../../../core/api/master.api';
import { AuthService } from '../../../core/auth/auth.service';
import { Notify } from '../../../core/ui/notify';
import { Barcode } from '../../../shared/barcode';
import { CostX100Pipe, RupiahPipe, bpToPercent, percentToBp } from '../../../shared/format';
import { DRUG_CLASSES, PRICE_MODES, drugClassInfo } from '../../../shared/labels';
import { ScanSelect } from '../../../shared/scan-select';

interface UnitRow {
  id: number | null;
  unitId: number | null;
  conversion: number | null;
  /** Barcode dipisah koma; scan barcode langsung menambah teks. */
  barcodes: string;
}

interface DataModel {
  code: string;
  name: string;
  genericName: string;
  manufacturerId: number | null;
  categoryId: number | null;
  drugClass: DrugClass;
  isOwa: boolean;
  minStockBase: number | null;
  rackId: number | null;
  /** Baris pertama selalu satuan dasar (isi 1). */
  units: UnitRow[];
  defaultIndex: number;
}

interface TierRow {
  minQty: number | null;
  priceMode: PriceMode;
  marginPercent: number | null;
  price: number | null;
}

interface UnitPriceRow {
  productUnitId: number;
  unitName: string;
  conversion: number;
  priceMode: PriceMode;
  sellPrice: number | null;
  tiers: TierRow[];
}

interface PriceModel {
  marginPercent: number | null;
  /** HPP acuan per satuan dasar, dalam rupiah (boleh desimal). */
  lastCost: number | null;
  units: UnitPriceRow[];
}

function emptyData(): DataModel {
  return {
    code: '',
    name: '',
    genericName: '',
    manufacturerId: null,
    categoryId: null,
    drugClass: 'FREE',
    isOwa: false,
    minStockBase: 0,
    rackId: null,
    units: [{ id: null, unitId: null, conversion: 1, barcodes: '' }],
    defaultIndex: 0,
  };
}

@Component({
  selector: 'app-product-dialog',
  imports: [
    FormsModule,
    ButtonModule,
    CheckboxModule,
    DialogModule,
    InputNumberModule,
    InputTextModule,
    PopoverModule,
    RadioButtonModule,
    SelectModule,
    TabsModule,
    TagModule,
    TooltipModule,
    RupiahPipe,
    CostX100Pipe,
    ScanSelect,
    Barcode,
  ],
  templateUrl: './product-dialog.html',
  styleUrl: './product-dialog.scss',
})
export class ProductDialog {
  private readonly notify = inject(Notify);
  protected readonly auth = inject(AuthService);

  /** `null` = obat baru. */
  readonly productId = input<number | null>(null);
  /** `true` bila ada perubahan yang tersimpan. */
  readonly closed = output<boolean>();

  protected readonly drugClasses = DRUG_CLASSES;
  protected readonly priceModes = PRICE_MODES;
  protected readonly canEditData = this.auth.can('PRODUCT_MANAGE');
  protected readonly canEditPrice = this.auth.can('PRICE_MANAGE');
  protected readonly canViewCost = this.auth.can('VIEW_COST');

  protected readonly product = signal<ProductDetail | null>(null);
  protected readonly units = signal<Unit[]>([]);
  protected readonly categories = signal<Category[]>([]);
  protected readonly racks = signal<NamedItem[]>([]);
  protected readonly manufacturers = signal<NamedItem[]>([]);
  protected readonly loading = signal(true);
  protected readonly saving = signal(false);
  protected readonly tab = signal<'data' | 'price'>('data');
  protected readonly visible = signal(true);

  protected data: DataModel = emptyData();
  protected price: PriceModel = { marginPercent: null, lastCost: null, units: [] };
  protected newUnitName = '';
  /** Baris satuan yang membuka popover jenis satuan baru; satuan baru langsung dipilih di baris ini. */
  private newUnitRow: number | null = null;
  private changed = false;

  protected readonly drugClass = computed(() => drugClassInfo(this.product()?.drugClass ?? 'FREE'));
  protected readonly unitsLocked = computed(() => this.product()?.hasStock ?? false);

  async ngOnInit(): Promise<void> {
    try {
      const id = this.productId();
      const [units, categories, racks, manufacturers, product] = await Promise.all([
        masterApi.unitList(),
        masterApi.categoryList(),
        masterApi.rackList(),
        masterApi.manufacturerList(),
        id == null ? Promise.resolve(null) : masterApi.productGet(id),
      ]);
      // Pilihan hanya data aktif, kecuali yang sedang dipakai obat ini.
      this.units.set(units);
      this.categories.set(categories.filter((c) => c.isActive || c.id === product?.categoryId));
      this.racks.set(racks.filter((r) => r.isActive || r.id === product?.rackId));
      this.manufacturers.set(manufacturers.filter((m) => m.isActive || m.id === product?.manufacturerId));
      if (product) {
        this.apply(product);
      }
    } catch (e) {
      this.notify.error(e);
      this.close();
    } finally {
      this.loading.set(false);
    }
  }

  // ─── Data obat ─────────────────────────────────────────────────────────────

  private apply(p: ProductDetail): void {
    this.product.set(p);
    const active = p.units.filter((u) => u.isActive).sort((a, b) => a.conversion - b.conversion);
    this.data = {
      code: p.code,
      name: p.name,
      genericName: p.genericName ?? '',
      manufacturerId: p.manufacturerId,
      categoryId: p.categoryId,
      drugClass: p.drugClass,
      isOwa: p.isOwa,
      minStockBase: p.minStockBase,
      rackId: p.rackId,
      units: active.map((u) => ({
        id: u.id,
        unitId: u.unitId,
        conversion: u.conversion,
        barcodes: u.barcodes.join(', '),
      })),
      defaultIndex: Math.max(0, active.findIndex((u) => u.isDefaultSale)),
    };
    this.price = {
      marginPercent: bpToPercent(p.marginBp),
      lastCost: p.lastCostX100 == null ? null : p.lastCostX100 / 100,
      units: active.map((u) => ({
        productUnitId: u.id,
        unitName: u.unitName,
        conversion: u.conversion,
        priceMode: u.priceMode,
        sellPrice: u.sellPrice,
        tiers: u.tiers.map((t) => ({
          minQty: t.minQty,
          priceMode: t.priceMode,
          marginPercent: bpToPercent(t.marginBp),
          price: t.price,
        })),
      })),
    };
  }

  protected unitName(unitId: number | null): string {
    return this.units().find((u) => u.id === unitId)?.name ?? '';
  }

  protected baseUnitName(): string {
    return this.unitName(this.data.units[0]?.unitId ?? null) || 'satuan dasar';
  }

  protected addUnitRow(): void {
    this.data.units.push({ id: null, unitId: null, conversion: null, barcodes: '' });
  }

  protected removeUnitRow(index: number): void {
    this.data.units.splice(index, 1);
    if (this.data.defaultIndex === index) {
      this.data.defaultIndex = 0;
    } else if (this.data.defaultIndex > index) {
      this.data.defaultIndex--;
    }
  }

  protected openNewUnit(event: Event, row: number, pop: Popover): void {
    this.newUnitRow = row;
    this.newUnitName = '';
    pop.toggle(event);
  }

  protected async createUnit(pop: Popover): Promise<void> {
    const name = this.newUnitName.trim();
    if (!name) return;
    try {
      const unit = await masterApi.unitCreate(name);
      this.units.update((list) => [...list, unit].sort((a, b) => a.name.localeCompare(b.name)));
      const row = this.newUnitRow != null ? this.data.units[this.newUnitRow] : undefined;
      if (row) row.unitId = unit.id;
      this.newUnitName = '';
      pop.hide();
      this.notify.success(`Satuan ${unit.name} ditambahkan`);
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected drugClassChanged(): void {
    if (this.data.drugClass !== 'HARD') {
      this.data.isOwa = false;
    }
  }

  protected async saveData(): Promise<void> {
    const d = this.data;
    const base = d.units[0];
    if (!d.name.trim() || base?.unitId == null) {
      this.notify.error('Nama obat dan satuan dasar wajib diisi');
      return;
    }
    if (d.units.some((u) => u.unitId == null || u.conversion == null)) {
      this.notify.error('Lengkapi jenis dan isi setiap satuan, atau hapus baris yang kosong');
      return;
    }

    this.saving.set(true);
    try {
      const result = await masterApi.productSave({
        id: this.product()?.id ?? null,
        name: d.name,
        genericName: d.genericName || null,
        manufacturerId: d.manufacturerId,
        categoryId: d.categoryId,
        drugClass: d.drugClass,
        isOwa: d.isOwa,
        baseUnitId: base.unitId,
        minStockBase: d.minStockBase ?? 0,
        rackId: d.rackId,
        units: d.units.map((u, i) => ({
          id: u.id,
          unitId: u.unitId!,
          conversion: i === 0 ? 1 : u.conversion!,
          isDefaultSale: i === d.defaultIndex,
          barcodes: u.barcodes
            .split(',')
            .map((b) => b.trim())
            .filter((b) => b),
        })),
      });
      this.changed = true;
      this.apply(result.product);
      this.notify.success('Data obat tersimpan');
      this.notify.warnings(result.warnings);
      if (this.canEditPrice) {
        this.tab.set('price');
      }
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  // ─── Harga ─────────────────────────────────────────────────────────────────

  protected unitCostX100(conversion: number): number | null {
    return this.price.lastCost == null ? null : Math.round(this.price.lastCost * 100) * conversion;
  }

  protected addTier(row: UnitPriceRow): void {
    const last = row.tiers.at(-1);
    row.tiers.push({ minQty: (last?.minQty ?? 1) + 4, priceMode: 'MANUAL', marginPercent: null, price: null });
  }

  protected removeTier(row: UnitPriceRow, index: number): void {
    row.tiers.splice(index, 1);
  }

  protected async savePrices(): Promise<void> {
    const p = this.product();
    if (!p) return;
    const incomplete = this.price.units.some(
      (u) =>
        (u.priceMode === 'MANUAL' && u.sellPrice == null) ||
        u.tiers.some(
          (t) => t.minQty == null || (t.priceMode === 'MANUAL' ? t.price == null : t.marginPercent == null),
        ),
    );
    if (incomplete) {
      this.notify.error('Lengkapi harga manual, jumlah minimal, dan margin/harga setiap tier');
      return;
    }

    this.saving.set(true);
    try {
      const result = await masterApi.productPricesSave({
        productId: p.id,
        marginBp: percentToBp(this.price.marginPercent),
        lastCostX100: this.price.lastCost == null ? null : Math.round(this.price.lastCost * 100),
        units: this.price.units.map((u) => ({
          productUnitId: u.productUnitId,
          priceMode: u.priceMode,
          sellPrice: u.sellPrice ?? 0,
          tiers: u.tiers.map((t) => ({
            minQty: t.minQty!,
            priceMode: t.priceMode,
            marginBp: percentToBp(t.marginPercent),
            price: t.price ?? 0,
          })),
        })),
      });
      this.changed = true;
      this.apply(result.product);
      this.notify.success('Harga tersimpan');
      this.notify.warnings(result.warnings);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  // ─── Status ────────────────────────────────────────────────────────────────

  protected async toggleActive(): Promise<void> {
    const p = this.product();
    if (!p) return;
    try {
      await masterApi.productSetActive(p.id, !p.isActive);
      this.changed = true;
      this.apply(await masterApi.productGet(p.id));
      this.notify.success(p.isActive ? 'Obat dinonaktifkan' : 'Obat diaktifkan kembali');
    } catch (e) {
      this.notify.error(e);
    }
  }

  protected close(): void {
    this.visible.set(false);
    this.closed.emit(this.changed);
  }
}
