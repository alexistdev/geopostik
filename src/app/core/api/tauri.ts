import { invoke } from '@tauri-apps/api/core';

import type { AppError } from '../../bindings/AppError';
import type { ErrorCode } from '../../bindings/ErrorCode';

/** Error dari command Rust, dengan pesan yang siap ditampilkan ke pengguna. */
export class ApiError extends Error {
  constructor(
    readonly code: ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

/** Memanggil command Rust. Semua error diubah menjadi `ApiError`. */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export function toApiError(e: unknown): ApiError {
  if (e instanceof ApiError) {
    return e;
  }
  if (isAppError(e)) {
    return new ApiError(e.code, e.message);
  }
  // Halaman dibuka di browser biasa, bukan di jendela Tauri.
  if (!isTauri()) {
    return new ApiError(
      'INTERNAL',
      'Tidak terhubung ke backend. Buka aplikasi di jendela GeoPOSTik ("npm run tauri dev"), bukan di browser.',
    );
  }
  if (typeof e === 'string') {
    return new ApiError('INTERNAL', e);
  }
  // Error lain (misal dari halaman tujuan setelah login) ditampilkan apa adanya agar mudah dilacak.
  console.error(e);
  const detail = e instanceof Error ? e.message : JSON.stringify(e);
  return new ApiError('INTERNAL', `Terjadi kesalahan: ${detail}`);
}

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    typeof (e as AppError).code === 'string' &&
    typeof (e as AppError).message === 'string'
  );
}
