import { Component, inject } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

import { AuthService } from '../../core/auth/auth.service';
import { REPORT_TABS } from './report-tabs';

/** Menu Laporan: tab hanya untuk laporan yang boleh dibuka user; Rust tetap memeriksa hak. */
@Component({
  selector: 'app-reports-page',
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  template: `
    <nav class="page-tabs">
      @for (t of tabs; track t.path) {
        <a [routerLink]="t.path" routerLinkActive="active"><i [class]="t.icon"></i>{{ t.label }}</a>
      }
    </nav>
    <router-outlet />
  `,
})
export class ReportsPage {
  private readonly auth = inject(AuthService);
  protected readonly tabs = REPORT_TABS.filter((t) => this.auth.can(t.permission));
}
