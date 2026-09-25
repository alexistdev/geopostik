import { describe, expect, it } from 'vitest';

import { ApiError, toApiError } from './tauri';

describe('toApiError', () => {
  it('keeps code and message from a Rust AppError', () => {
    const err = toApiError({ code: 'VALIDATION', message: 'PIN harus 4–6 digit angka' });
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe('VALIDATION');
    expect(err.message).toBe('PIN harus 4–6 digit angka');
  });

  it('wraps plain string errors as INTERNAL', () => {
    const err = toApiError('command not found');
    expect(err.code).toBe('INTERNAL');
    expect(err.message).toBe('command not found');
  });

  it('explains missing backend when running outside Tauri', () => {
    const err = toApiError(new TypeError("Cannot read properties of undefined (reading 'invoke')"));
    expect(err.code).toBe('INTERNAL');
    expect(err.message).toContain('npm run tauri dev');
  });
});
