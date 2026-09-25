import type { Category } from '../../bindings/Category';
import type { CategoryInput } from '../../bindings/CategoryInput';
import type { ProductDetail } from '../../bindings/ProductDetail';
import type { ProductInput } from '../../bindings/ProductInput';
import type { ProductListQuery } from '../../bindings/ProductListQuery';
import type { ProductListResult } from '../../bindings/ProductListResult';
import type { ProductPricesInput } from '../../bindings/ProductPricesInput';
import type { ProductSaveResult } from '../../bindings/ProductSaveResult';
import type { Unit } from '../../bindings/Unit';
import { call } from './tauri';

export const masterApi = {
  categoryList: () => call<Category[]>('category_list'),
  categorySave: (input: CategoryInput) => call<Category>('category_save', { input }),
  unitList: () => call<Unit[]>('unit_list'),
  unitCreate: (name: string) => call<Unit>('unit_create', { name }),
  productList: (query: ProductListQuery) => call<ProductListResult>('product_list', { query }),
  productGet: (id: number) => call<ProductDetail>('product_get', { id }),
  productSave: (input: ProductInput) => call<ProductSaveResult>('product_save', { input }),
  productPricesSave: (input: ProductPricesInput) =>
    call<ProductSaveResult>('product_prices_save', { input }),
  productSetActive: (id: number, active: boolean) => call<void>('product_set_active', { id, active }),
};
