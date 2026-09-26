import type { DebtPage } from '../../bindings/DebtPage';
import type { DebtQuery } from '../../bindings/DebtQuery';
import type { MasterPageQuery } from '../../bindings/MasterPageQuery';
import type { PurchaseDefaults } from '../../bindings/PurchaseDefaults';
import type { PurchaseDetail } from '../../bindings/PurchaseDetail';
import type { PurchaseInput } from '../../bindings/PurchaseInput';
import type { PurchasePage } from '../../bindings/PurchasePage';
import type { PurchasePostInput } from '../../bindings/PurchasePostInput';
import type { PurchaseProduct } from '../../bindings/PurchaseProduct';
import type { PurchaseQuery } from '../../bindings/PurchaseQuery';
import type { PurchaseSaveResult } from '../../bindings/PurchaseSaveResult';
import type { Supplier } from '../../bindings/Supplier';
import type { SupplierInput } from '../../bindings/SupplierInput';
import type { SupplierPage } from '../../bindings/SupplierPage';
import type { SupplierPaymentInput } from '../../bindings/SupplierPaymentInput';
import { call } from './tauri';

export const purchasingApi = {
  supplierList: () => call<Supplier[]>('supplier_list'),
  supplierPage: (query: MasterPageQuery) => call<SupplierPage>('supplier_page', { query }),
  supplierSave: (input: SupplierInput) => call<Supplier>('supplier_save', { input }),
  supplierSetActive: (id: number, active: boolean) => call<void>('supplier_set_active', { id, active }),
  supplierDelete: (id: number) => call<void>('supplier_delete', { id }),
  defaults: () => call<PurchaseDefaults>('purchase_defaults'),
  page: (query: PurchaseQuery) => call<PurchasePage>('purchase_page', { query }),
  get: (id: number) => call<PurchaseDetail>('purchase_get', { id }),
  productSearch: (q: string) => call<PurchaseProduct[]>('purchase_product_search', { q }),
  save: (input: PurchaseInput) => call<PurchaseSaveResult>('purchase_save', { input }),
  post: (input: PurchasePostInput) => call<PurchaseSaveResult>('purchase_post', { input }),
  void: (id: number, reason: string | null) => call<PurchaseDetail>('purchase_void', { id, reason }),
  debtPage: (query: DebtQuery) => call<DebtPage>('debt_page', { query }),
  paymentCreate: (input: SupplierPaymentInput) => call<PurchaseDetail>('supplier_payment_create', { input }),
  paymentVoid: (id: number, reason: string) => call<PurchaseDetail>('supplier_payment_void', { id, reason }),
};
