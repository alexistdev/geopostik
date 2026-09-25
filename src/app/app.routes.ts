import { Routes } from '@angular/router';

import { authGuard, guestGuard, permissionGuard, setupGuard } from './core/auth/guards';
import { NAV_ITEMS } from './layout/nav';

const BUILT = ['dashboard', 'obat', 'master-data'];

// Menu yang belum punya halaman sendiri memakai Placeholder.
const pendingRoutes: Routes = NAV_ITEMS.filter((item) => !BUILT.includes(item.path)).map((item) => ({
  path: item.path,
  canActivate: item.permission ? [permissionGuard(item.permission)] : [],
  data: { title: item.label },
  loadComponent: () => import('./features/placeholder/placeholder').then((m) => m.Placeholder),
}));

export const routes: Routes = [
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
        ],
      },
      ...pendingRoutes,
    ],
  },
  { path: '**', redirectTo: '' },
];
