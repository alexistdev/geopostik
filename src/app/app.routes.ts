import { Routes } from '@angular/router';

import { activationGuard, authGuard, guestGuard, permissionGuard, setupGuard } from './core/auth/guards';
import { firstReportTab } from './features/reports/report-tabs';
import { NAV_ITEMS } from './layout/nav';

const BUILT = ['dashboard', 'kasir', 'obat', 'master-data', 'resep', 'pembelian', 'stok', 'laporan', 'pengguna', 'pengaturan', 'sistem/log'];

// Menu yang belum punya halaman sendiri memakai Placeholder.
const pendingRoutes: Routes = NAV_ITEMS.filter((item) => !BUILT.includes(item.path)).map((item) => ({
  path: item.path,
  canActivate: item.permission ? [permissionGuard(item.permission)] : [],
  data: { title: item.label },
  loadComponent: () => import('./features/placeholder/placeholder').then((m) => m.Placeholder),
}));

export const routes: Routes = [
  {
    path: 'aktivasi',
    canActivate: [activationGuard],
    loadComponent: () => import('./features/license/activation').then((m) => m.Activation),
  },
  {
    path: 'setup',
    canActivate: [setupGuard],
    loadComponent: () => import('./features/auth/setup/setup').then((m) => m.Setup),
  },
  {
    path: 'login',
    canActivate: [guestGuard],
    loadComponent: () => import('./features/auth/login/login').then((m) => m.Login),
  },
  {
    path: '',
    canActivate: [authGuard],
    loadComponent: () => import('./layout/shell').then((m) => m.Shell),
    children: [
      { path: '', pathMatch: 'full', redirectTo: 'dashboard' },
      {
        path: 'dashboard',
        loadComponent: () => import('./features/dashboard/dashboard').then((m) => m.Dashboard),
      },
      {
        path: 'kasir',
        canActivate: [permissionGuard('SALE_CREATE')],
        loadComponent: () => import('./features/pos/pos-page').then((m) => m.PosPage),
      },
      {
        path: 'resep',
        canActivate: [permissionGuard('PRESCRIPTION_INPUT')],
        loadComponent: () =>
          import('./features/prescription/prescription-list').then((m) => m.PrescriptionList),
      },
      {
        path: 'obat',
        canActivate: [permissionGuard('PRODUCT_MANAGE')],
        loadComponent: () => import('./features/master/products/product-list').then((m) => m.ProductList),
      },
      {
        path: 'master-data',
        canActivate: [permissionGuard('PRODUCT_MANAGE')],
        loadComponent: () => import('./features/master/master-data-page').then((m) => m.MasterDataPage),
        children: [
          { path: '', pathMatch: 'full', redirectTo: 'kategori' },
          {
            path: 'kategori',
            loadComponent: () =>
              import('./features/master/categories/category-list').then((m) => m.CategoryList),
          },
          {
            path: 'rak',
            data: { kind: 'rack' },
            loadComponent: () => import('./features/master/named/named-list').then((m) => m.NamedList),
          },
          {
            path: 'pabrik',
            data: { kind: 'manufacturer' },
            loadComponent: () => import('./features/master/named/named-list').then((m) => m.NamedList),
          },
          {
            path: 'dokter',
            loadComponent: () => import('./features/master/doctors/doctor-list').then((m) => m.DoctorList),
          },
          {
            path: 'pasien',
            loadComponent: () => import('./features/master/patients/patient-list').then((m) => m.PatientList),
          },
          {
            path: 'supplier',
            loadComponent: () => import('./features/master/suppliers/supplier-list').then((m) => m.SupplierList),
          },
        ],
      },
      {
        path: 'pembelian',
        canActivate: [permissionGuard('PURCHASE_RECEIVE')],
        loadComponent: () => import('./features/purchasing/purchasing-page').then((m) => m.PurchasingPage),
        children: [
          { path: '', pathMatch: 'full', redirectTo: 'faktur' },
          {
            path: 'faktur',
            loadComponent: () => import('./features/purchasing/purchase-list').then((m) => m.PurchaseList),
          },
          {
            path: 'faktur/:id',
            loadComponent: () => import('./features/purchasing/purchase-form').then((m) => m.PurchaseForm),
          },
          {
            path: 'hutang',
            canActivate: [permissionGuard('SUPPLIER_DEBT_MANAGE')],
            loadComponent: () => import('./features/purchasing/debt-list').then((m) => m.DebtList),
          },
        ],
      },
      {
        path: 'pengguna',
        canActivate: [permissionGuard('USER_MANAGE')],
        loadComponent: () => import('./features/users/user-list').then((m) => m.UserList),
      },
      {
        path: 'stok',
        canActivate: [permissionGuard('STOCK_COUNT_INPUT')],
        loadComponent: () => import('./features/inventory/stock-page').then((m) => m.StockPage),
        children: [
          { path: '', pathMatch: 'full', redirectTo: 'daftar' },
          {
            path: 'daftar',
            loadComponent: () => import('./features/inventory/stock-list').then((m) => m.StockList),
          },
          {
            path: 'kartu',
            loadComponent: () => import('./features/inventory/stock-card').then((m) => m.StockCard),
          },
          {
            path: 'opname',
            loadComponent: () => import('./features/inventory/opname-list').then((m) => m.OpnameList),
          },
          {
            path: 'opname/:id',
            loadComponent: () => import('./features/inventory/opname-detail').then((m) => m.OpnameDetail),
          },
        ],
      },
      {
        path: 'laporan',
        canActivate: [permissionGuard('REPORT_SALES_OWN_SHIFT')],
        loadComponent: () => import('./features/reports/reports-page').then((m) => m.ReportsPage),
        children: [
          { path: '', pathMatch: 'full', canActivate: [firstReportTab], children: [] },
          {
            path: 'penjualan',
            loadComponent: () => import('./features/reports/sales-report').then((m) => m.SalesReport),
          },
          {
            path: 'obat',
            canActivate: [permissionGuard('REPORT_SALES')],
            loadComponent: () => import('./features/reports/product-report').then((m) => m.ProductReport),
          },
          {
            path: 'persediaan',
            canActivate: [permissionGuard('VIEW_COST')],
            loadComponent: () => import('./features/reports/inventory-report').then((m) => m.InventoryReport),
          },
          {
            path: 'ed',
            canActivate: [permissionGuard('STOCK_COUNT_INPUT')],
            loadComponent: () => import('./features/reports/expiry-report').then((m) => m.ExpiryReport),
          },
          {
            path: 'pembelian',
            canActivate: [permissionGuard('PURCHASE_RECEIVE')],
            loadComponent: () => import('./features/reports/purchase-report').then((m) => m.PurchaseReport),
          },
          {
            path: 'sipnap',
            canActivate: [permissionGuard('REPORT_SIPNAP')],
            loadComponent: () => import('./features/reports/sipnap-report').then((m) => m.SipnapReport),
          },
        ],
      },
      {
        path: 'pengaturan',
        canActivate: [permissionGuard('SETTINGS_MANAGE')],
        loadComponent: () => import('./features/settings/settings').then((m) => m.Settings),
      },
      {
        path: 'sistem/log',
        canActivate: [permissionGuard('AUDIT_VIEW')],
        loadComponent: () => import('./features/system/audit-log').then((m) => m.AuditLog),
      },
      ...pendingRoutes,
    ],
  },
  { path: '**', redirectTo: '' },
];
