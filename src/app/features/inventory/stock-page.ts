import { Component } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

/** Menu Stok: stok per batch + ED, kartu stok, dan stok opname (stok awal & berkala). */
@Component({
  selector: 'app-stock-page',
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <nav class="page-tabs">
      <a routerLink="daftar" routerLinkActive="active"><i class="pi pi-warehouse"></i>Stok</a>
      <a routerLink="kartu" routerLinkActive="active"><i class="pi pi-book"></i>Kartu Stok</a>
      <a routerLink="opname" routerLinkActive="active"><i class="pi pi-check-square"></i>Stok Opname</a>
    </nav>
    <router-outlet />
  `,
})
export class StockPage {}
