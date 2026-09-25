import type { AppStatus } from '../../bindings/AppStatus';
import type { LoginInput } from '../../bindings/LoginInput';
import type { SessionUser } from '../../bindings/SessionUser';
import type { SetupInput } from '../../bindings/SetupInput';
import { call } from './tauri';

export const authApi = {
  appStatus: () => call<AppStatus>('app_status'),
  setupOwner: (input: SetupInput) => call<SessionUser>('setup_owner', { input }),
  login: (input: LoginInput) => call<SessionUser>('login', { input }),
  logout: () => call<void>('logout'),
};
