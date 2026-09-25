import { Component, inject } from '@angular/core';

import { AuthService } from '../../core/auth/auth.service';

@Component({
  selector: 'app-dashboard',
  template: `
    <h2>Selamat datang, {{ auth.user()?.fullName }}</h2>
    <p>Ringkasan omzet, obat hampir ED, dan stok menipis akan tampil di sini.</p>
  `,
})
export class Dashboard {
  protected readonly auth = inject(AuthService);
}
