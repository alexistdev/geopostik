import { signal } from '@angular/core';

import type { MasterPageQuery } from '../../bindings/MasterPageQuery';

export const MASTER_PAGE_SIZES = [25, 50, 100];
export const PAGE_REPORT = 'Menampilkan {first}–{last} dari {totalRecords}';

interface Page<T> {
  rows: T[];
  total: number;
}

/**
 * Status tabel Master Data yang dipaginasi di server: halaman, jumlah baris, pencarian
 * (dengan jeda ketik), dan baris yang dicentang untuk cetak label.
 */
export class MasterTable<T extends { id: number; code: string; name: string }> {
  readonly rows = signal<T[]>([]);
  readonly total = signal(0);
  readonly loading = signal(false);
  readonly first = signal(0);
  readonly pageSize = signal(MASTER_PAGE_SIZES[0]);
  readonly search = signal('');
  selected: T[] = [];

  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly fetchPage: (query: MasterPageQuery) => Promise<Page<T>>,
    private readonly fetchAll: () => Promise<T[]>,
    private readonly onError: (e: unknown) => void,
  ) {}

  /** Dipanggil p-table (onLazyLoad) saat pindah halaman atau ganti jumlah baris. */
  pageChanged(first: number, rows: number): void {
    this.pageSize.set(rows);
    this.load(first);
  }

  searchChanged(value: string): void {
    this.search.set(value);
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.load(0), 250);
  }

  async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await this.fetchPage({
        q: this.search().trim() || null,
        offset: first,
        limit: this.pageSize(),
      });
      // Halaman jadi kosong (misal setelah hapus baris terakhir) → mundur satu halaman.
      if (page.rows.length === 0 && first > 0) {
        return this.load(Math.max(0, first - this.pageSize()));
      }
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
    } catch (e) {
      this.onError(e);
    } finally {
      this.loading.set(false);
    }
  }

  /** Baris yang dicentang, atau semua data yang cocok dengan pencarian (semua halaman). */
  async itemsToPrint(): Promise<T[]> {
    if (this.selected.length) {
      return this.selected;
    }
    const q = this.search().trim().toLowerCase();
    const all = await this.fetchAll();
    return q ? all.filter((i) => i.name.toLowerCase().includes(q) || i.code.toLowerCase().includes(q)) : all;
  }
}
