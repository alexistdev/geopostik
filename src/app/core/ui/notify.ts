import { Injectable, inject } from '@angular/core';
import { MessageService } from 'primeng/api';

import { toApiError } from '../api/tauri';

/** Notifikasi toast (ditampilkan oleh <p-toast> di layout utama). */
@Injectable({ providedIn: 'root' })
export class Notify {
  private readonly messages = inject(MessageService);

  success(detail: string): void {
    this.messages.add({ severity: 'success', summary: 'Berhasil', detail, life: 2500 });
  }

  /** Peringatan yang tidak menggagalkan proses, misal harga di bawah HPP. */
  warnings(list: string[]): void {
    for (const detail of list) {
      this.messages.add({ severity: 'warn', summary: 'Perhatian', detail, life: 6000 });
    }
  }

  error(e: unknown): void {
    this.messages.add({ severity: 'error', summary: 'Gagal', detail: toApiError(e).message, life: 6000 });
  }
}
