import type { BatchResult } from '../../bindings/BatchResult';
import type { UserInput } from '../../bindings/UserInput';
import type { UserListQuery } from '../../bindings/UserListQuery';
import type { UserListResult } from '../../bindings/UserListResult';
import type { UserRow } from '../../bindings/UserRow';
import type { UserSaveResult } from '../../bindings/UserSaveResult';
import { call } from './tauri';

export const userApi = {
  list: (query: UserListQuery) => call<UserListResult>('user_list', { query }),
  get: (id: number) => call<UserRow>('user_get', { id }),
  save: (input: UserInput) => call<UserSaveResult>('user_save', { input }),
  setActive: (id: number, active: boolean) => call<void>('user_set_active', { id, active }),
  delete: (id: number) => call<void>('user_delete', { id }),
  setActiveMany: (ids: number[], active: boolean) => call<BatchResult>('user_set_active_many', { ids, active }),
  deleteMany: (ids: number[]) => call<BatchResult>('user_delete_many', { ids }),
};
