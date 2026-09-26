import type { PosPrescription } from '../../bindings/PosPrescription';
import type { PosProduct } from '../../bindings/PosProduct';
import type { PosState } from '../../bindings/PosState';
import type { PrescriptionQuote } from '../../bindings/PrescriptionQuote';
import type { SaleDetail } from '../../bindings/SaleDetail';
import type { SaleInput } from '../../bindings/SaleInput';
import type { SalePage } from '../../bindings/SalePage';
import type { SaleQuery } from '../../bindings/SaleQuery';
import type { SaleVoidInput } from '../../bindings/SaleVoidInput';
import type { ShiftCloseInput } from '../../bindings/ShiftCloseInput';
import type { ShiftOpenInput } from '../../bindings/ShiftOpenInput';
import type { ShiftSummary } from '../../bindings/ShiftSummary';
import { call } from './tauri';

export const salesApi = {
  state: () => call<PosState>('pos_state'),
  shiftOpen: (input: ShiftOpenInput) => call<ShiftSummary>('shift_open', { input }),
  shiftClose: (input: ShiftCloseInput) => call<ShiftSummary>('shift_close', { input }),
  search: (q: string) => call<PosProduct[]>('pos_search', { q }),
  create: (input: SaleInput) => call<SaleDetail>('sale_create', { input }),
  get: (id: number) => call<SaleDetail>('sale_get', { id }),
  page: (query: SaleQuery) => call<SalePage>('sale_page', { query }),
  void: (input: SaleVoidInput) => call<SaleDetail>('sale_void', { input }),
  prescriptions: () => call<PosPrescription[]>('pos_prescriptions'),
  prescriptionQuote: (id: number) => call<PrescriptionQuote>('pos_prescription_quote', { id }),
};
