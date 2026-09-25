import { Component, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import {
  AbstractControl,
  FormBuilder,
  ReactiveFormsModule,
  ValidationErrors,
  Validators,
} from '@angular/forms';
import { Router } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { InputTextModule } from 'primeng/inputtext';
import { MessageModule } from 'primeng/message';

import { toApiError } from '../../../core/api/tauri';
import { AuthService } from '../../../core/auth/auth.service';

function passwordsMatch(group: AbstractControl): ValidationErrors | null {
  const { password, passwordConfirm } = group.value as { password: string; passwordConfirm: string };
  return password === passwordConfirm ? null : { passwordMismatch: true };
}

/** Setup awal: dijalankan sekali saat database masih kosong, membuat akun pemilik. */
@Component({
  selector: 'app-setup',
  imports: [ReactiveFormsModule, ButtonModule, CheckboxModule, InputTextModule, MessageModule],
  templateUrl: './setup.html',
  styleUrl: '../auth-page.scss',
})
export class Setup {
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);

  protected readonly form = inject(FormBuilder).nonNullable.group(
    {
      pharmacyName: ['', Validators.required],
      fullName: ['', Validators.required],
      username: ['', [Validators.required, Validators.pattern(/^[A-Za-z0-9._]{3,32}$/)]],
      password: ['', [Validators.required, Validators.minLength(6)]],
      passwordConfirm: ['', Validators.required],
      pin: ['', [Validators.required, Validators.pattern(/^\d{4,6}$/)]],
      alsoPharmacist: [false],
      licenseNumber: [''],
    },
    { validators: passwordsMatch },
  );
  protected readonly alsoPharmacist = toSignal(this.form.controls.alsoPharmacist.valueChanges, {
    initialValue: false,
  });
  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);

  protected invalid(name: keyof typeof this.form.controls): boolean {
    const c = this.form.controls[name];
    return c.invalid && c.touched;
  }

  protected async submit(): Promise<void> {
    if (this.form.invalid || this.loading()) {
      this.form.markAllAsTouched();
      return;
    }
    const v = this.form.getRawValue();
    this.loading.set(true);
    this.error.set(null);
    try {
      await this.auth.setupOwner({
        pharmacyName: v.pharmacyName,
        fullName: v.fullName,
        username: v.username,
        password: v.password,
        pin: v.pin,
        alsoPharmacist: v.alsoPharmacist,
        licenseNumber: v.alsoPharmacist ? v.licenseNumber : null,
      });
      await this.router.navigateByUrl('/');
    } catch (e) {
      this.error.set(toApiError(e).message);
    } finally {
      this.loading.set(false);
    }
  }
}
