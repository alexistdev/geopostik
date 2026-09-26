import type { TaxSettings } from '../../bindings/TaxSettings';
import { call } from './tauri';

export const settingsApi = {
  taxGet: () => call<TaxSettings>('tax_settings_get'),
  taxSave: (input: TaxSettings) => call<TaxSettings>('tax_settings_save', { input }),
};
