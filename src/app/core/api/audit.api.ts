import type { AuditPage } from '../../bindings/AuditPage';
import type { AuditQuery } from '../../bindings/AuditQuery';
import type { AuditUser } from '../../bindings/AuditUser';
import { call } from './tauri';

export const auditApi = {
  page: (query: AuditQuery) => call<AuditPage>('audit_page', { query }),
  users: () => call<AuditUser[]>('audit_users'),
};
