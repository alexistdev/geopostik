import { Component } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

/** Menu Master Data: data pendukung yang dipilih saat tambah/ubah obat. */
@Component({
  selector: 'app-master-data-page',
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <nav class="tabs">
      <a routerLink="kategori" routerLinkActive="active"><i class="pi pi-tags"></i>Kategori</a>
      <a routerLink="rak" routerLinkActive="active"><i class="pi pi-th-large"></i>Rak</a>
      <a routerLink="pabrik" routerLinkActive="active"><i class="pi pi-building"></i>Pabrik</a>
    </nav>
    <router-outlet />
  `,
  styles: `
    // Tab model map: sisi kanan miring, aktif hijau tua, lainnya abu-abu.
    .tabs {
      --tab-slant: 18px;
      --tab-bg: #d9d9d9;
      --tab-bg-hover: #cccccc;
      --tab-active: var(--p-green-800);

      display: flex;
      align-items: flex-end;
      gap: 4px;
      margin-bottom: 1rem;
      border-bottom: 1px solid var(--tab-bg);

      a {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.55rem calc(1.25rem + var(--tab-slant)) 0.55rem 1.25rem;
        font-size: 0.85rem;
        font-weight: 600;
        color: var(--tab-active);
        background: var(--tab-bg);
        border-radius: 6px 0 0 0;
        text-decoration: none;
        clip-path: polygon(0 0, calc(100% - var(--tab-slant)) 0, 100% 100%, 0 100%);
        transition: background-color 0.15s;

        i {
          font-size: 0.85rem;
        }

        &:hover:not(.active) {
          background: var(--tab-bg-hover);
        }

        &.active {
          color: #fff;
          background: var(--tab-active);
        }
      }
    }
  `,
})
export class MasterDataPage {}
