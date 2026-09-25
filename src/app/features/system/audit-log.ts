import { Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { IconFieldModule } from 'primeng/iconfield';
import { InputIconModule } from 'primeng/inputicon';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToggleSwitchModule } from 'primeng/toggleswitch';

import type { AuditRow } from '../../bindings/AuditRow';
import type { AuditUser } from '../../bindings/AuditUser';
import { auditApi } from '../../core/api/audit.api';
import { Notify } from '../../core/ui/notify';
import { ACTIONS, ENTITIES, actionInfo, diffSnapshots, entityLabel, fieldLabel, formatValue, parseDetail, summarize } from './audit-format';

const PAGE_SIZES = [50, 100, 200];

/** Menu Sistem › Log: audit log semua aktivitas (hanya baca, tidak bisa diubah atau dihapus). */
@Component({
  selector: 'app-audit-log',
  imports: [
    FormsModule,
    ButtonModule,
    DialogModule,
    IconFieldModule,
    InputIconModule,
    InputTextModule,
    SelectModule,
    TableModule,
    TagModule,
    ToggleSwitchModule,
  ],
  templateUrl: './audit-log.html',
  styleUrl: './audit-log.scss',
})
export class AuditLog {
  private readonly notify = inject(Notify);

  protected readonly entities = ENTITIES;
  protected readonly actions = ACTIONS;
  protected readonly pageSizes = PAGE_SIZES;
  protected readonly actionInfo = actionInfo;
  protected readonly entityLabel = entityLabel;
  protected readonly summarize = summarize;
  protected readonly fieldLabel = fieldLabel;
  protected readonly formatValue = formatValue;

  protected readonly rows = signal<AuditRow[]>([]);
  protected readonly total = signal(0);
  protected readonly loading = signal(false);
  protected readonly first = signal(0);
  protected readonly users = signal<AuditUser[]>([]);
  protected pageSize = PAGE_SIZES[0];

  protected q = '';
  protected entity: string | null = null;
  protected action: string | null = null;
  protected userId: number | null = null;
  protected dateFrom = '';
  protected dateTo = '';

  protected readonly selected = signal<AuditRow | null>(null);
  /** Detail: tampilkan hanya field yang berubah (default) atau isi lengkap. */
  protected readonly changedOnly = signal(true);
  protected readonly showRaw = signal(false);
  protected readonly detail = computed(() => {
    const row = this.selected();
    return row ? parseDetail(row) : null;
  });
  protected readonly changes = computed(() => {
    const d = this.detail();
    if (!d?.hasSnapshot) return [];
    const all = diffSnapshots(d.before, d.after);
    // Data baru / terhapus tidak punya pembanding: selalu tampilkan isi lengkap.
    const compare = d.before != null && d.after != null;
    return compare && this.changedOnly() ? all.filter((c) => c.changed) : all;
  });

  private searchTimer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    auditApi
      .users()
      .then((u) => this.users.set(u))
      .catch((e) => this.notify.error(e));
  }

  protected pageChanged(first: number, rows: number): void {
    this.pageSize = rows;
    this.load(first);
  }

  protected searchChanged(): void {
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.load(0), 250);
  }

  protected get hasFilter(): boolean {
    return !!(this.q.trim() || this.entity || this.action || this.userId || this.dateFrom || this.dateTo);
  }

  protected resetFilter(): void {
    this.q = '';
    this.entity = this.action = null;
    this.userId = null;
    this.dateFrom = this.dateTo = '';
    this.load(0);
  }

  protected async load(first = this.first()): Promise<void> {
    this.loading.set(true);
    try {
      const page = await auditApi.page({
        q: this.q.trim() || null,
        entity: this.entity,
        action: this.action,
        userId: this.userId,
        dateFrom: this.dateFrom || null,
        dateTo: this.dateTo || null,
        offset: first,
        limit: this.pageSize,
      });
      this.first.set(first);
      this.rows.set(page.rows);
      this.total.set(page.total);
    } catch (e) {
      this.notify.error(e);
    } finally {
      this.loading.set(false);
    }
  }

  protected open(row: AuditRow): void {
    this.changedOnly.set(true);
    this.showRaw.set(false);
    this.selected.set(row);
  }

  protected codeName(row: AuditRow): string {
    const d = parseDetail(row);
    return [d.code, d.name].filter(Boolean).join(' · ') || (row.entityId != null ? `#${row.entityId}` : '–');
  }

  protected prettyRaw(raw: string | null): string {
    if (!raw) return '–';
    try {
      return JSON.stringify(JSON.parse(raw), null, 2);
    } catch {
      return raw;
    }
  }

  protected extraEntries(extra: Record<string, unknown>): { label: string; value: string }[] {
    return Object.entries(extra).map(([k, v]) => ({ label: fieldLabel([k]), value: formatValue(k, v) }));
  }
}
