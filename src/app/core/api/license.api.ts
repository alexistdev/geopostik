import type { LicenseStatus } from '../../bindings/LicenseStatus';
import { call } from './tauri';

export const licenseApi = {
  status: () => call<LicenseStatus>('license_status'),
  activate: (licenseKey: string) => call<LicenseStatus>('license_activate', { licenseKey }),
  revalidate: () => call<LicenseStatus>('license_revalidate'),
};
