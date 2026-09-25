import type { Permission } from '../bindings/Permission';

export interface NavItem {
  label: string;
  icon: string;
  path: string;
  /** Hak yang dibutuhkan; kosong = semua user yang login. */
  permission?: Permission;
  /** Tombol pintas keyboard yang tampil di menu. */
  hotkey?: string;
}

/** Menu utama. Rute & guard dibuat dari daftar ini (lihat app.routes.ts). */
export const NAV_ITEMS: NavItem[] = [
  { label: 'Dashboard', icon: 'pi pi-home', path: 'dashboard' },
  { label: 'Kasir', icon: 'pi pi-shopping-cart', path: 'kasir', permission: 'SALE_CREATE', hotkey: 'F2' },
  { label: 'Resep', icon: 'pi pi-file-edit', path: 'resep', permission: 'PRESCRIPTION_INPUT' },
  { label: 'Obat', icon: 'pi pi-box', path: 'obat', permission: 'PRODUCT_MANAGE' },
  { label: 'Master Data', icon: 'pi pi-database', path: 'master-data', permission: 'PRODUCT_MANAGE' },
  { label: 'Pembelian', icon: 'pi pi-truck', path: 'pembelian', permission: 'PURCHASE_RECEIVE' },
  { label: 'Stok', icon: 'pi pi-warehouse', path: 'stok', permission: 'STOCK_COUNT_INPUT' },
  { label: 'Laporan', icon: 'pi pi-chart-bar', path: 'laporan', permission: 'REPORT_SALES_OWN_SHIFT' },
  { label: 'Pengguna', icon: 'pi pi-users', path: 'pengguna', permission: 'USER_MANAGE' },
  { label: 'Pengaturan', icon: 'pi pi-cog', path: 'pengaturan', permission: 'SETTINGS_MANAGE' },
];
