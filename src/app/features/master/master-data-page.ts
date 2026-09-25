import { Component } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

/** Menu Master Data: data pendukung yang dipilih saat tambah/ubah obat. */
@Component({
  selector: 'app-master-data-page',
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <nav class="tabs">
      <a routerLink="kategori" routerLinkActive="active">Kategori</a>
      <a routerLink="rak" routerLinkActive="active">Rak</a>
      <a routerLink="pabrik" routerLinkActive="active">Pabrik</a>
    </nav>
    <router-outlet />
  `,
  styles: `
    .tabs {
      display: flex;
      gap: 0.25rem;
      margin-bottom: 1rem;
      border-bottom: 1px solid var(--p-surface-200);

      a {
        padding: 0.6rem 1rem;
        color: var(--p-text-muted-color);
        text-decoration: none;
        border-bottom: 2px solid transparent;
        margin-bottom: -1px;

        &.active {
          color: var(--p-primary-700);
          border-bottom-color: var(--p-primary-600);
          font-weight: 600;
        }
      }
    }
  `,
})
export class MasterDataPage {}
