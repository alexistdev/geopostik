import { Component, DestroyRef, computed, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router } from '@angular/router';

import { toApiError } from '../../../core/api/tauri';
import { AuthService } from '../../../core/auth/auth.service';
import { LicenseService } from '../../../core/license/license.service';

const timeFormat = new Intl.DateTimeFormat('id-ID', { hour: '2-digit', minute: '2-digit' });
const dateFormat = new Intl.DateTimeFormat('id-ID', {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
  year: 'numeric',
});

function greeting(hour: number): string {
  if (hour < 4) return 'Selamat malam';
  if (hour < 11) return 'Selamat pagi';
  if (hour < 15) return 'Selamat siang';
  if (hour < 18) return 'Selamat sore';
  return 'Selamat malam';
}

@Component({
  selector: 'app-login',
  imports: [ReactiveFormsModule],
  templateUrl: './login.html',
  styleUrl: './login.scss',
})
export class Login {
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);
  protected readonly license = inject(LicenseService);

  protected readonly form = inject(FormBuilder).nonNullable.group({
    username: ['', Validators.required],
    password: ['', Validators.required],
  });
  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Dinaikkan tiap gagal login agar animasi goyang diputar ulang. */
  protected readonly errorCount = signal(0);
  protected readonly showPassword = signal(false);
  protected readonly capsLock = signal(false);

  private readonly now = signal(new Date());
  protected readonly time = computed(() => timeFormat.format(this.now()));
  protected readonly date = computed(() => dateFormat.format(this.now()));
  protected readonly greeting = computed(() => greeting(this.now().getHours()));
  protected readonly pharmacyName = this.auth.pharmacyName;

  constructor() {
    const timer = setInterval(() => this.now.set(new Date()), 1000);
    inject(DestroyRef).onDestroy(() => clearInterval(timer));
  }

  protected invalid(name: 'username' | 'password'): boolean {
    const c = this.form.controls[name];
    return c.invalid && c.touched;
  }

  protected checkCapsLock(event: KeyboardEvent): void {
    this.capsLock.set(event.getModifierState?.('CapsLock') ?? false);
  }

  protected async submit(): Promise<void> {
    if (this.form.invalid || this.loading()) {
      this.form.markAllAsTouched();
      return;
    }
    this.loading.set(true);
    this.error.set(null);
    try {
      await this.auth.login(this.form.getRawValue());
      await this.router.navigateByUrl('/');
    } catch (e) {
      this.error.set(toApiError(e).message);
      this.errorCount.update((n) => n + 1);
      this.form.controls.password.reset();
    } finally {
      this.loading.set(false);
    }
  }
}
