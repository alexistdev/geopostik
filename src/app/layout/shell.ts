import { Component, computed, inject } from '@angular/core';
import { takeUntilDestroyed, toSignal } from '@angular/core/rxjs-interop';
import { NavigationEnd, Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { filter, map } from 'rxjs';
import { ButtonModule } from 'primeng/button';
import { ConfirmDialogModule } from 'primeng/confirmdialog';
import { ToastModule } from 'primeng/toast';

import { AuthService } from '../core/auth/auth.service';
import { LicenseService } from '../core/license/license.service';
import { LicenseGate } from '../features/license/license-gate';
import { NAV_ITEMS } from './nav';

@Component({
  selector: 'app-shell',
  imports: [RouterOutlet, RouterLink, RouterLinkActive, ButtonModule, ConfirmDialogModule, ToastModule, LicenseGate],
  templateUrl: './shell.html',
  styleUrl: './shell.scss',
})
export class Shell {
  private readonly router = inject(Router);
  protected readonly auth = inject(AuthService);
  protected readonly license = inject(LicenseService);

  private readonly url = toSignal(
    this.router.events.pipe(
      filter((e) => e instanceof NavigationEnd),
      map(() => this.router.url),
    ),
    { initialValue: this.router.url },
  );

  /** License tidak aktif: semua halaman selain Pengaturan diganti kotak license. */
  protected readonly locked = computed(
    () => !this.license.active() && !this.url().startsWith('/pengaturan'),
  );

  constructor() {
    // Masa aktif bisa habis saat aplikasi terbuka: periksa ulang file license tiap pindah menu.
    this.router.events
      .pipe(
        filter((e) => e instanceof NavigationEnd),
        takeUntilDestroyed(),
      )
      .subscribe(() => this.license.refresh().catch(() => undefined));
  }

  protected readonly navItems = computed(() => {
    this.auth.user(); // hitung ulang saat user berganti
    return NAV_ITEMS.filter((item) => !item.permission || this.auth.can(item.permission));
  });

  protected async logout(): Promise<void> {
    await this.auth.logout();
    await this.router.navigateByUrl('/login');
  }
}
