import { ChangeDetectorRef, Component, OnInit, computed, inject, input, output, signal } from '@angular/core';
import { NgTemplateOutlet } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ConfirmationService } from 'primeng/api';
import { AutoCompleteModule, type AutoCompleteCompleteEvent } from 'primeng/autocomplete';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { InputNumberModule } from 'primeng/inputnumber';
import { InputTextModule } from 'primeng/inputtext';
import { MessageModule } from 'primeng/message';
import { SelectModule } from 'primeng/select';
import { TagModule } from 'primeng/tag';
import { TextareaModule } from 'primeng/textarea';
import { TooltipModule } from 'primeng/tooltip';

import type { CompoundForm } from '../../bindings/CompoundForm';
import type { Doctor } from '../../bindings/Doctor';
import type { DrugClass } from '../../bindings/DrugClass';
import type { Patient } from '../../bindings/Patient';
import type { PrescriptionDetail } from '../../bindings/PrescriptionDetail';
import type { PrescriptionItemDetail } from '../../bindings/PrescriptionItemDetail';
import type { PrescriptionItemInput } from '../../bindings/PrescriptionItemInput';
import type { ProductDetail } from '../../bindings/ProductDetail';
import type { ProductListRow } from '../../bindings/ProductListRow';
import type { ProductUnitDetail } from '../../bindings/ProductUnitDetail';
import { masterApi } from '../../core/api/master.api';
import { prescriptionApi } from '../../core/api/prescription.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe } from '../../shared/format';
import { drugClassInfo } from '../../shared/labels';
import { DoctorDialog } from '../master/doctors/doctor-dialog';
import { PatientDialog } from '../master/patients/patient-dialog';
import { COMPOUND_FORMS, ageFromBirthDate, compoundFormInfo, formatDate, statusInfo, todayIso } from './rx-labels';

/** Obat yang dipilih di resep, beserta satuan jual & harganya (untuk hitung perkiraan harga). */
interface ProductRef {
  productId: number;
  code: string;
  name: string;
  drugClass: DrugClass;
  units: ProductUnitDetail[];
  /** Nama satuan terkecil, untuk menampilkan stok. */
  baseUnit: string;
}

interface DrugLine extends ProductRef {
  key: number;
  productUnitId: number;
  qty: number | null;
}

interface ProductItem extends DrugLine {
  kind: 'PRODUCT';
  usage: string;
}

interface CompoundItem {
  kind: 'COMPOUND';
  key: number;
  name: string;
  form: CompoundForm;
  qty: number | null;
  usage: string;
  components: DrugLine[];
}

interface ServiceItem {
  kind: 'SERVICE';
  key: number;
  name: string;
  qty: number | null;
  price: number | null;
}

type Item = ProductItem | CompoundItem | ServiceItem;

interface Header {
  prescriptionNumber: string;
  prescriptionDate: string;
  doctorId: number | null;
  customerId: number | null;
  patientName: string;
  patientAge: string;
  patientAddress: string;
  note: string;
}

const SCREENING_CHECKS = [
  { key: 'admin', label: 'Administratif', hint: 'Data dokter (nama, SIP, paraf), pasien, dan tanggal resep lengkap.' },
  { key: 'pharma', label: 'Farmasetik', hint: 'Bentuk sediaan, kekuatan, stabilitas, dan kompatibilitas racikan.' },
  { key: 'clinical', label: 'Klinis', hint: 'Indikasi, dosis, aturan pakai, duplikasi, interaksi, alergi.' },
] as const;

/** Tambah / ubah / lihat resep, termasuk racikan, jasa, skrining apoteker, dan batal. */
@Component({
  selector: 'app-prescription-dialog',
  imports: [
    NgTemplateOutlet,
    FormsModule,
    AutoCompleteModule,
    ButtonModule,
    CheckboxModule,
    DialogModule,
    InputNumberModule,
    InputTextModule,
    MessageModule,
    SelectModule,
    TagModule,
    TextareaModule,
    TooltipModule,
    DateTimePipe,
    RupiahPipe,
    DoctorDialog,
    PatientDialog,
  ],
  templateUrl: './prescription-dialog.html',
  styleUrl: './prescription-dialog.scss',
})
export class PrescriptionDialog implements OnInit {
  private readonly notify = inject(Notify);
  private readonly cdr = inject(ChangeDetectorRef);
  private readonly auth = inject(AuthService);
  private readonly confirm = inject(ConfirmationService);

  /** `null` = resep baru. */
  readonly prescriptionId = input<number | null>(null);
  /** `true` bila ada perubahan tersimpan. */
  readonly closed = output<boolean>();

  protected readonly canInput = this.auth.can('PRESCRIPTION_INPUT');
  protected readonly canValidate = this.auth.can('PRESCRIPTION_VALIDATE');
  protected readonly compoundForms = COMPOUND_FORMS;
  protected readonly compoundFormInfo = compoundFormInfo;
  protected readonly drugClassInfo = drugClassInfo;
  protected readonly statusInfo = statusInfo;
  protected readonly formatDate = formatDate;
  protected readonly screeningChecks = SCREENING_CHECKS;
  protected readonly today = todayIso();

  protected readonly detail = signal<PrescriptionDetail | null>(null);
  protected readonly loading = signal(true);
  protected readonly saving = signal(false);
  protected readonly doctors = signal<Doctor[]>([]);
  /** Dialog tambahan: dokter baru, pasien baru, skrining, batal. */
  protected readonly sub = signal<'doctor' | 'patient' | 'screen' | 'cancel' | null>(null);

  protected readonly editable = computed(() => {
    const d = this.detail();
    return this.canInput && (!d || d.status === 'DRAFT' || d.status === 'SCREENED');
  });
  protected readonly doctorOptions = computed(() => {
    const current = this.detail()?.doctorId;
    return this.doctors().filter((d) => d.isActive || d.id === current);
  });

  protected rx: Header = emptyHeader();
  protected items: Item[] = [];
  /** Ada perubahan yang belum disimpan (untuk peringatan saat menutup). */
  protected dirty = false;
  private changed = false;
  private nextKey = 1;
  private readonly productCache = new Map<number, ProductRef>();

  // Pencarian obat (baris utama & komponen racikan) dan pasien.
  protected productSuggestions: ProductListRow[] = [];
  protected productQuery: ProductListRow | string | null = null;
  protected componentQuery: Record<number, ProductListRow | string | null> = {};
  protected patientSuggestions: Patient[] = [];
  protected patientQuery: Patient | string | null = null;

  protected screening = { admin: false, pharma: false, clinical: false, note: '' };
  protected cancelReason = '';

  async ngOnInit(): Promise<void> {
    prescriptionApi.doctorList().then(
      (d) => this.doctors.set(d),
      (e) => this.notify.error(e),
    );
    const id = this.prescriptionId();
    if (id != null) {
      try {
        await this.apply(await prescriptionApi.get(id));
      } catch (e) {
        this.notify.error(e);
        this.closed.emit(false);
      }
    }
    this.loading.set(false);
  }

  /** Isi form dari data tersimpan. */
  private async apply(d: PrescriptionDetail): Promise<void> {
    const ids = new Set<number>();
    for (const i of d.items) {
      if (i.productId != null) ids.add(i.productId);
      i.components.forEach((c) => c.productId != null && ids.add(c.productId));
    }
    await Promise.all([...ids].map((pid) => this.productRef(pid)));

    this.rx = {
      prescriptionNumber: d.prescriptionNumber,
      prescriptionDate: d.prescriptionDate,
      doctorId: d.doctorId,
      customerId: d.customerId,
      patientName: d.patientName,
      patientAge: d.patientAge ?? '',
      patientAddress: d.patientAddress ?? '',
      note: d.note ?? '',
    };
    this.patientQuery = d.customerId ? d.patientName : null;
    this.items = d.items.map((i) => this.fromDetail(i));
    this.detail.set(d);
    this.dirty = false;
    this.cdr.markForCheck();
  }

  private fromDetail(i: PrescriptionItemDetail): Item {
    const drug = (c: PrescriptionItemDetail): DrugLine => ({
      ...this.productCache.get(c.productId!)!,
      key: this.nextKey++,
      productUnitId: c.productUnitId!,
      qty: c.qty,
    });
    switch (i.kind) {
      case 'PRODUCT':
        return { ...drug(i), kind: 'PRODUCT', usage: i.usageInstruction ?? '' };
      case 'COMPOUND':
        return {
          kind: 'COMPOUND',
          key: this.nextKey++,
          name: i.description,
          form: i.compoundForm ?? 'OTHER',
          qty: i.qty,
          usage: i.usageInstruction ?? '',
          components: i.components.map(drug),
        };
      case 'SERVICE':
        return { kind: 'SERVICE', key: this.nextKey++, name: i.description, qty: i.qty, price: i.unitPrice };
    }
  }

  private async productRef(productId: number): Promise<ProductRef> {
    const cached = this.productCache.get(productId);
    if (cached) return cached;
    const p: ProductDetail = await masterApi.productGet(productId);
    const ref: ProductRef = {
      productId: p.id,
      code: p.code,
      name: p.name,
      drugClass: p.drugClass,
      // Satuan nonaktif tetap dimuat agar resep lama yang memakainya tetap tampil.
      units: p.units,
      baseUnit: p.units.find((u) => u.conversion === 1)?.unitName ?? '',
    };
    this.productCache.set(productId, ref);
    return ref;
  }

  // ─── Pencarian ───────────────────────────────────────────────────────────

  protected async searchProducts(event: AutoCompleteCompleteEvent): Promise<void> {
    try {
      const result = await masterApi.productList({
        q: event.query,
        categoryId: null,
        drugClass: null,
        includeInactive: false,
        offset: 0,
        limit: 15,
      });
      this.productSuggestions = result.rows;
    } catch (e) {
      this.notify.error(e);
      this.productSuggestions = [];
    }
    this.cdr.markForCheck();
  }

  private async newDrugLine(row: ProductListRow): Promise<DrugLine | null> {
    try {
      const ref = await this.productRef(row.id);
      const active = ref.units.filter((u) => u.isActive);
      const unit = active.find((u) => u.isDefaultSale) ?? active[0];
      if (!unit) {
        this.notify.warnings([`${ref.name} belum punya satuan jual aktif`]);
        return null;
      }
      return { ...ref, key: this.nextKey++, productUnitId: unit.id, qty: 1 };
    } catch (e) {
      this.notify.error(e);
      return null;
    }
  }

  protected async addProduct(row: ProductListRow): Promise<void> {
    this.productQuery = null;
    const line = await this.newDrugLine(row);
    if (line) {
      this.items = [...this.items, { ...line, kind: 'PRODUCT', usage: '' }];
      this.touch();
    }
    this.cdr.markForCheck();
  }

  protected async addComponent(compound: CompoundItem, row: ProductListRow): Promise<void> {
    this.componentQuery[compound.key] = null;
    const line = await this.newDrugLine(row);
    if (line) {
      // Komponen racikan umumnya dihitung per satuan terkecil (tablet/kapsul).
      const base = line.units.find((u) => u.isActive && u.conversion === 1);
      if (base) line.productUnitId = base.id;
      compound.components = [...compound.components, line];
      this.touch();
    }
    this.cdr.markForCheck();
  }

  protected async searchPatients(event: AutoCompleteCompleteEvent): Promise<void> {
    try {
      const page = await prescriptionApi.patientPage({ q: event.query, offset: 0, limit: 15 }, true);
      this.patientSuggestions = page.rows;
    } catch (e) {
      this.notify.error(e);
      this.patientSuggestions = [];
    }
    this.cdr.markForCheck();
  }

  protected selectPatient(p: Patient): void {
    this.rx.customerId = p.id;
    this.rx.patientName = p.name;
    this.rx.patientAge = ageFromBirthDate(p.birthDate) ?? this.rx.patientAge;
    this.rx.patientAddress = p.address ?? this.rx.patientAddress;
    this.patientQuery = p.name;
    this.touch();
  }

  protected clearPatient(): void {
    this.rx.customerId = null;
    this.patientQuery = null;
    this.touch();
  }

  protected doctorCreated(d: Doctor): void {
    this.doctors.update((list) => [...list, d].sort((a, b) => a.name.localeCompare(b.name)));
    this.rx.doctorId = d.id;
    this.sub.set(null);
    this.touch();
  }

  protected patientCreated(p: Patient): void {
    this.sub.set(null);
    this.selectPatient(p);
  }

  protected newPatientInitial() {
    return { name: this.rx.patientName, address: this.rx.patientAddress || null };
  }

  // ─── Baris ───────────────────────────────────────────────────────────────

  protected addCompound(): void {
    const n = this.items.filter((i) => i.kind === 'COMPOUND').length + 1;
    this.items = [
      ...this.items,
      { kind: 'COMPOUND', key: this.nextKey++, name: `Racikan ${n}`, form: 'POWDER', qty: 10, usage: '', components: [] },
    ];
    this.touch();
  }

  protected addService(name: string): void {
    this.items = [...this.items, { kind: 'SERVICE', key: this.nextKey++, name, qty: 1, price: null }];
    this.touch();
  }

  protected removeItem(item: Item): void {
    this.items = this.items.filter((i) => i !== item);
    this.touch();
  }

  protected removeComponent(compound: CompoundItem, line: DrugLine): void {
    compound.components = compound.components.filter((c) => c !== line);
    this.touch();
  }

  protected touch(): void {
    this.dirty = true;
  }

  protected unitOptions(line: DrugLine): ProductUnitDetail[] {
    return line.units.filter((u) => u.isActive || u.id === line.productUnitId);
  }

  protected unitOf(line: DrugLine): ProductUnitDetail | undefined {
    return line.units.find((u) => u.id === line.productUnitId);
  }

  /** Harga per satuan: tier grosir dengan jumlah minimal terbesar yang tercapai, atau harga eceran. */
  protected priceOf(line: DrugLine): { price: number; tier: number | null } {
    const unit = this.unitOf(line);
    if (!unit) return { price: 0, tier: null };
    const qty = line.qty ?? 0;
    const tier = unit.tiers.filter((t) => t.minQty <= qty).sort((a, b) => b.minQty - a.minQty)[0];
    return tier ? { price: tier.price, tier: tier.minQty } : { price: unit.sellPrice, tier: null };
  }

  protected lineTotal(line: DrugLine): number {
    return this.priceOf(line).price * (line.qty ?? 0);
  }

  protected itemTotal(item: Item): number {
    switch (item.kind) {
      case 'PRODUCT':
        return this.lineTotal(item);
      case 'COMPOUND':
        return item.components.reduce((sum, c) => sum + this.lineTotal(c), 0);
      case 'SERVICE':
        return (item.price ?? 0) * (item.qty ?? 0);
    }
  }

  protected total(): number {
    return this.items.reduce((sum, i) => sum + this.itemTotal(i), 0);
  }

  /** Stok terjual-bisa dari data tersimpan (hanya untuk baris yang sudah disimpan). */
  protected savedStock(productId: number): number | null {
    const d = this.detail();
    if (!d) return null;
    for (const i of d.items) {
      if (i.productId === productId) return i.stockBase;
      const c = i.components.find((x) => x.productId === productId);
      if (c) return c.stockBase;
    }
    return null;
  }

  protected hasControlled(): boolean {
    const controlled = (c: DrugClass) => c === 'NARCOTIC' || c === 'PSYCHOTROPIC';
    return this.items.some(
      (i) =>
        (i.kind === 'PRODUCT' && controlled(i.drugClass)) ||
        (i.kind === 'COMPOUND' && i.components.some((c) => controlled(c.drugClass))),
    );
  }

  protected hasHard(): boolean {
    return this.items.some(
      (i) =>
        (i.kind === 'PRODUCT' && i.drugClass === 'HARD') ||
        (i.kind === 'COMPOUND' && i.components.some((c) => c.drugClass === 'HARD')),
    );
  }

  protected itemCount(): number {
    return this.items.length;
  }

  // ─── Simpan, skrining, batal ─────────────────────────────────────────────

  private toInput(item: Item): PrescriptionItemInput {
    const none = { productUnitId: null, description: null, compoundForm: null, unitPrice: null, usageInstruction: null };
    switch (item.kind) {
      case 'PRODUCT':
        return {
          ...none,
          kind: 'PRODUCT',
          productUnitId: item.productUnitId,
          qty: item.qty ?? 0,
          usageInstruction: item.usage,
          components: [],
        };
      case 'COMPOUND':
        return {
          ...none,
          kind: 'COMPOUND',
          qty: item.qty ?? 0,
          description: item.name,
          compoundForm: item.form,
          usageInstruction: item.usage,
          components: item.components.map((c) => ({ productUnitId: c.productUnitId, qty: c.qty ?? 0 })),
        };
      case 'SERVICE':
        return { ...none, kind: 'SERVICE', qty: item.qty ?? 0, description: item.name, unitPrice: item.price, components: [] };
    }
  }

  protected async save(): Promise<void> {
    const h = this.rx;
    if (h.doctorId == null) {
      this.notify.warnings(['Pilih dokter penulis resep']);
      return;
    }
    this.saving.set(true);
    try {
      const wasScreened = this.detail()?.status === 'SCREENED';
      const saved = await prescriptionApi.save({
        id: this.detail()?.id ?? null,
        prescriptionNumber: h.prescriptionNumber,
        prescriptionDate: h.prescriptionDate,
        doctorId: h.doctorId,
        customerId: h.customerId,
        patientName: h.patientName,
        patientAge: h.patientAge || null,
        patientAddress: h.patientAddress || null,
        note: h.note || null,
        items: this.items.map((i) => this.toInput(i)),
      });
      this.changed = true;
      await this.apply(saved);
      this.notify.success(
        wasScreened ? `Resep ${saved.number} tersimpan dan perlu divalidasi ulang` : `Resep ${saved.number} tersimpan`,
      );
      this.notify.warnings(saved.warnings);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  protected openScreening(): void {
    this.screening = { admin: false, pharma: false, clinical: false, note: '' };
    this.sub.set('screen');
  }

  protected screeningReady(): boolean {
    return this.screening.admin && this.screening.pharma && this.screening.clinical;
  }

  protected async screen(): Promise<void> {
    const d = this.detail();
    if (!d) return;
    this.saving.set(true);
    try {
      const result = await prescriptionApi.screen({ id: d.id, note: this.screening.note || null });
      this.changed = true;
      this.sub.set(null);
      await this.apply(result);
      this.notify.success(`Resep ${result.number} divalidasi, siap dibayar di kasir`);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  protected openCancel(): void {
    this.cancelReason = '';
    this.sub.set('cancel');
  }

  protected async cancelPrescription(): Promise<void> {
    const d = this.detail();
    if (!d) return;
    this.saving.set(true);
    try {
      const result = await prescriptionApi.cancel(d.id, this.cancelReason);
      this.changed = true;
      this.sub.set(null);
      await this.apply(result);
      this.notify.success(`Resep ${result.number} dibatalkan`);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.saving.set(false);
    }
  }

  protected close(): void {
    if (!this.dirty || !this.editable()) {
      this.closed.emit(this.changed);
      return;
    }
    this.confirm.confirm({
      header: 'Tutup tanpa menyimpan?',
      message: 'Perubahan pada resep ini belum disimpan dan akan hilang.',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Tutup',
      rejectLabel: 'Kembali',
      acceptButtonProps: { severity: 'danger' },
      rejectButtonProps: { severity: 'secondary', outlined: true },
      accept: () => this.closed.emit(this.changed),
    });
  }
}

function emptyHeader(): Header {
  return {
    prescriptionNumber: '',
    prescriptionDate: todayIso(),
    doctorId: null,
    customerId: null,
    patientName: '',
    patientAge: '',
    patientAddress: '',
    note: '',
  };
}
