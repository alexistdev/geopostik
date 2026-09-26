import { Component, computed, inject, signal } from '@angular/core';
import { RouterLink } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { SkeletonModule } from 'primeng/skeleton';
import { TooltipModule } from 'primeng/tooltip';

import type { Dashboard as DashboardData } from '../../bindings/Dashboard';
import type { Permission } from '../../bindings/Permission';
import type { Role } from '../../bindings/Role';
import { dashboardApi } from '../../core/api/dashboard.api';
import { AuthService } from '../../core/auth/auth.service';
import { Notify } from '../../core/ui/notify';
import { DateTimePipe, RupiahPipe, formatRupiah } from '../../shared/format';

type Section = 'shift' | 'sales' | 'rx' | 'stock' | 'opname' | 'debts';
type Tone = 'neutral' | 'good' | 'warn' | 'danger';

interface Tile {
  label: string;
  value: string;
  sub?: string;
  icon: string;
  tone: Tone;
  link?: string;
}

/**
 * Urutan bagian menurut peran utama (peran pertama yang cocok dari atas). Bagian yang tidak
 * dikirim Rust (tidak berhak) otomatis hilang; bagian lain yang ada ditambahkan di belakang.
 */
const ROLE_ORDER: [Role, Section[]][] = [
  ['PHARMACIST', ['rx', 'stock', 'sales', 'opname', 'shift']],
  ['OWNER', ['sales', 'debts', 'stock', 'opname', 'rx', 'shift']],
  ['TECHNICIAN', ['rx', 'stock', 'opname', 'shift']],
  ['CASHIER', ['shift', 'rx']],
];
const ALL_SECTIONS: Section[] = ['shift', 'sales', 'rx', 'stock', 'opname', 'debts'];

const METHOD_LABELS: Record<string, string> = { CASH: 'Tunai', QRIS: 'QRIS', DEBIT: 'Debit' };
const DAY_SHORT = new Intl.DateTimeFormat('id-ID', { weekday: 'short' });
const DAY_LONG = new Intl.DateTimeFormat('id-ID', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
const COMPACT = new Intl.NumberFormat('id-ID', { notation: 'compact', maximumFractionDigits: 1 });

/** `YYYY-MM-DD` → Date lokal (tanpa geser zona waktu). */
function localDate(ymd: string): Date {
  const [y, m, d] = ymd.split('-').map(Number);
  return new Date(y, m - 1, d);
}

/** Dashboard: isi per peran. Rust hanya mengirim panel yang boleh dilihat user. */
@Component({
  selector: 'app-dashboard',
  imports: [RouterLink, ButtonModule, SkeletonModule, TooltipModule, RupiahPipe, DateTimePipe],
  templateUrl: './dashboard.html',
  styleUrl: './dashboard.scss',
})
export class Dashboard {
  protected readonly auth = inject(AuthService);
  private readonly notify = inject(Notify);

  protected readonly data = signal<DashboardData | null>(null);
  protected readonly loading = signal(false);
  /** Indeks batang grafik 7 hari yang sedang di-hover. */
  protected readonly hoverDay = signal<number | null>(null);

  protected readonly methodLabels = METHOD_LABELS;

  protected readonly greeting = computed(() => {
    const h = new Date().getHours();
    return h < 11 ? 'Selamat pagi' : h < 15 ? 'Selamat siang' : h < 18 ? 'Selamat sore' : 'Selamat malam';
  });

  protected readonly todayLabel = computed(() => {
    const d = this.data();
    return DAY_LONG.format(d ? localDate(d.today) : new Date());
  });

  protected readonly sections = computed<Section[]>(() => {
    const d = this.data();
    if (!d) return [];
    const roles = this.auth.user()?.roles ?? [];
    const preferred = ROLE_ORDER.find(([role]) => roles.includes(role))?.[1] ?? [];
    const present = (s: Section) =>
      ({
        shift: d.shift,
        sales: d.sales,
        rx: d.prescriptions,
        stock: d.stock,
        opname: d.opname,
        debts: d.debts,
      })[s] != null;
    return [...new Set([...preferred, ...ALL_SECTIONS])].filter(present);
  });

  /** Kartu angka di atas: hanya dari panel yang dikirim. */
  protected readonly tiles = computed<Tile[]>(() => {
    const d = this.data();
    if (!d) return [];
    const tiles: Tile[] = [];

    if (d.sales) {
      const s = d.sales;
      tiles.push({
        label: 'Omzet hari ini',
        value: formatRupiah(s.today.amount),
        sub: `${s.today.count} nota · ${this.delta(s.today.amount, s.yesterday.amount)}`,
        icon: 'pi pi-wallet',
        tone: 'neutral',
      });
      tiles.push({
        label: 'Omzet bulan ini',
        value: formatRupiah(s.month.amount),
        sub: `${s.month.count} nota`,
        icon: 'pi pi-calendar',
        tone: 'neutral',
      });
      if (s.grossProfitToday != null) {
        tiles.push({
          label: 'Laba kotor hari ini',
          value: formatRupiah(s.grossProfitToday),
          sub: `Bulan ini ${formatRupiah(s.grossProfitMonth)}`,
          icon: 'pi pi-chart-line',
          tone: s.grossProfitToday < 0 ? 'danger' : 'good',
        });
      }
    } else if (d.shift) {
      tiles.push({
        label: 'Penjualan saya hari ini',
        value: formatRupiah(d.shift.mySalesToday.amount),
        sub: `${d.shift.mySalesToday.count} nota`,
        icon: 'pi pi-wallet',
        tone: 'neutral',
      });
    }

    if (d.prescriptions) {
      const rx = d.prescriptions;
      if (rx.queueStatus === 'DRAFT') {
        tiles.push({
          label: 'Resep menunggu skrining',
          value: String(rx.awaitingScreening),
          sub: `${rx.readyToPay} siap dibayar`,
          icon: 'pi pi-file-check',
          tone: rx.awaitingScreening > 0 ? 'warn' : 'good',
          link: this.linkIf('PRESCRIPTION_INPUT', '/resep'),
        });
      } else {
        tiles.push({
          label: 'Resep siap dibayar',
          value: String(rx.readyToPay),
          sub: 'Sudah divalidasi apoteker',
          icon: 'pi pi-file-check',
          tone: rx.readyToPay > 0 ? 'warn' : 'good',
          link: this.linkIf('SALE_CREATE', '/kasir'),
        });
      }
    }

    if (d.stock) {
      const st = d.stock;
      const stockLink = this.linkIf('STOCK_COUNT_INPUT', '/stok/daftar');
      tiles.push({
        label: 'Stok di bawah minimal',
        value: String(st.lowCount),
        sub: `${st.emptyCount} obat habis`,
        icon: 'pi pi-arrow-down',
        tone: st.lowCount > 0 ? 'warn' : 'good',
        link: stockLink,
      });
      tiles.push({
        label: 'Batch hampir / sudah ED',
        value: `${st.nearExpiryCount} / ${st.expiredCount}`,
        sub: 'ED ≤ 3 bulan / lewat ED',
        icon: 'pi pi-clock',
        tone: st.expiredCount > 0 ? 'danger' : st.nearExpiryCount > 0 ? 'warn' : 'good',
        link: stockLink,
      });
    }

    if (d.debts) {
      const db = d.debts;
      tiles.push({
        label: 'Hutang lewat jatuh tempo',
        value: formatRupiah(db.overdue.amount),
        sub: `${db.overdue.count} faktur · ${db.dueSoon.count} jatuh tempo ≤ 7 hari`,
        icon: 'pi pi-credit-card',
        tone: db.overdue.count > 0 ? 'danger' : db.dueSoon.count > 0 ? 'warn' : 'good',
        link: this.linkIf('SUPPLIER_DEBT_MANAGE', '/pembelian/hutang'),
      });
    }
    return tiles;
  });

  /** Grafik 7 hari: tinggi batang relatif terhadap hari tertinggi. */
  protected readonly week = computed(() => {
    const daily = this.data()?.sales?.daily ?? [];
    const max = Math.max(1, ...daily.map((d) => d.amount));
    return daily.map((d, i) => ({
      ...d,
      day: DAY_SHORT.format(localDate(d.date)),
      pct: (d.amount / max) * 100,
      isToday: i === daily.length - 1,
      short: d.amount > 0 ? COMPACT.format(d.amount) : '',
    }));
  });

  protected readonly methodTotal = computed(() =>
    (this.data()?.sales?.byMethodToday ?? []).reduce((sum, m) => sum + m.amount, 0),
  );

  constructor() {
    this.load();
  }

  protected async load(): Promise<void> {
    this.loading.set(true);
    try {
      this.data.set(await dashboardApi.get());
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected can(p: Permission): boolean {
    return this.auth.can(p);
  }

  private linkIf(p: Permission, path: string): string | undefined {
    return this.auth.can(p) ? path : undefined;
  }

  private delta(now: number, before: number): string {
    if (before === 0) return now > 0 ? 'kemarin Rp0' : 'sama dengan kemarin';
    const pct = Math.round(((now - before) / before) * 100);
    return pct === 0 ? 'sama dengan kemarin' : `${pct > 0 ? '▲' : '▼'} ${Math.abs(pct)}% dari kemarin`;
  }

  protected daysLabel(days: number): string {
    if (days < 0) return `lewat ${-days} hari`;
    if (days === 0) return 'hari ini';
    return `${days} hari lagi`;
  }

  protected dueLabel(days: number): string {
    if (days < 0) return `terlambat ${-days} hari`;
    if (days === 0) return 'jatuh tempo hari ini';
    return `${days} hari lagi`;
  }

  protected shortDate(ymd: string): string {
    const [y, m, d] = ymd.split('-');
    return `${d}/${m}/${y}`;
  }
}
