import { Directive, inject } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Select } from 'primeng/select';

/**
 * Untuk `<p-select [filter]="true">` yang bisa diisi lewat scan barcode.
 *
 * Scanner mengetik kode lalu menekan Enter. Bawaan PrimeNG mengosongkan sorotan opsi setiap
 * kali filter berubah, sehingga Enter tidak memilih apa pun. Directive ini menyorot hasil
 * teratas setelah filter berubah, jadi Enter langsung memilihnya.
 */
@Directive({ selector: 'p-select[appScanSelect]' })
export class ScanSelect {
  private readonly select = inject(Select);

  constructor() {
    this.select.onFilter.pipe(takeUntilDestroyed()).subscribe(() => {
      if (this.select.visibleOptions().length > 0) {
        this.select.focusedOptionIndex.set(0);
      }
    });
  }
}
