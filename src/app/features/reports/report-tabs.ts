import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';

import type { Permission } from '../../bindings/Permission';
import { AuthService } from '../../core/auth/auth.service';

export interface ReportTab {
  path: string;
  label: string;
  icon: string;
  permission: Permission;
}

/** Tab menu Laporan (FLOW.md bagian J). Kartu stok dan log audit ada di menunya sendiri. */
export const REPORT_TABS: ReportTab[] = [
  { path: 'penjualan', label: 'Penjualan', icon: 'pi pi-wallet', permission: 'REPORT_SALES_OWN_SHIFT' },
  { path: 'obat', label: 'Obat Terlaris', icon: 'pi pi-star', permission: 'REPORT_SALES' },
  { path: 'persediaan', label: 'Nilai Persediaan', icon: 'pi pi-warehouse', permission: 'VIEW_COST' },
  { path: 'ed', label: 'Kedaluwarsa', icon: 'pi pi-clock', permission: 'STOCK_COUNT_INPUT' },
  { path: 'pembelian', label: 'Pembelian', icon: 'pi pi-truck', permission: 'PURCHASE_RECEIVE' },
  { path: 'sipnap', label: 'Narkotika & Psikotropika', icon: 'pi pi-shield', permission: 'REPORT_SIPNAP' },
];

/** `/laporan` → tab pertama yang boleh dibuka user. */
export const firstReportTab: CanActivateFn = () => {
  const auth = inject(AuthService);
  const tab = REPORT_TABS.find((t) => auth.can(t.permission));
  return inject(Router).parseUrl(tab ? `/laporan/${tab.path}` : '/');
};
