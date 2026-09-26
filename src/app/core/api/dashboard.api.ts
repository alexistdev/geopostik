import type { Dashboard } from '../../bindings/Dashboard';
import { call } from './tauri';

export const dashboardApi = {
  get: () => call<Dashboard>('dashboard_get'),
};
