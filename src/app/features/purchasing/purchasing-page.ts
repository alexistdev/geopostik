import { Component, inject } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

import { AuthService } from '../../core/auth/auth.service';

/** Menu Pembelian: faktur penerimaan barang dan hutang supplier. */
@Component({
  selector: 'app-purchasing-page',
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <nav class="page-tabs">
      <a routerLink="faktur" routerLinkActive="active"><i class="pi pi-truck"></i>Penerimaan Barang</a>
      @if (canDebt) {
        <a routerLink="hutang" routerLinkActive="active"><i class="pi pi-credit-card"></i>Hutang Supplier</a>
      }
    </nav>
    <router-outlet />
  `,
})
export class PurchasingPage {
  protected readonly canDebt = inject(AuthService).can('SUPPLIER_DEBT_MANAGE');
}
