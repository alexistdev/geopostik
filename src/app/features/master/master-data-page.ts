import { Component } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

/** Menu Master Data: data pendukung yang dipilih saat tambah/ubah obat. */
@Component({
  selector: 'app-master-data-page',
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <nav class="page-tabs">
      <a routerLink="kategori" routerLinkActive="active"><i class="pi pi-tags"></i>Kategori</a>
      <a routerLink="rak" routerLinkActive="active"><i class="pi pi-th-large"></i>Rak</a>
      <a routerLink="pabrik" routerLinkActive="active"><i class="pi pi-building"></i>Pabrik</a>
      <a routerLink="dokter" routerLinkActive="active"><i class="pi pi-user"></i>Dokter</a>
      <a routerLink="pasien" routerLinkActive="active"><i class="pi pi-heart"></i>Pasien</a>
      <a routerLink="supplier" routerLinkActive="active"><i class="pi pi-truck"></i>Supplier</a>
    </nav>
    <router-outlet />
  `,
})
export class MasterDataPage {}
