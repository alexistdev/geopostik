import type { Category } from '../../bindings/Category';
import type { CategoryInput } from '../../bindings/CategoryInput';
import type { CategoryPage } from '../../bindings/CategoryPage';
import type { MasterKind } from '../../bindings/MasterKind';
import type { MasterPageQuery } from '../../bindings/MasterPageQuery';
import type { NamedItem } from '../../bindings/NamedItem';
import type { NamedItemInput } from '../../bindings/NamedItemInput';
import type { NamedItemPage } from '../../bindings/NamedItemPage';
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
  categoryPage: (query: MasterPageQuery) => call<CategoryPage>('category_page', { query }),
  categorySave: (input: CategoryInput) => call<Category>('category_save', { input }),
  rackList: () => call<NamedItem[]>('rack_list'),
  rackPage: (query: MasterPageQuery) => call<NamedItemPage>('rack_page', { query }),
  rackSave: (input: NamedItemInput) => call<NamedItem>('rack_save', { input }),
  manufacturerList: () => call<NamedItem[]>('manufacturer_list'),
  manufacturerPage: (query: MasterPageQuery) => call<NamedItemPage>('manufacturer_page', { query }),
  manufacturerSave: (input: NamedItemInput) => call<NamedItem>('manufacturer_save', { input }),
  masterSetActive: (kind: MasterKind, id: number, active: boolean) =>
    call<void>('master_set_active', { kind, id, active }),
  masterDelete: (kind: MasterKind, id: number) => call<void>('master_delete', { kind, id }),
  unitList: () => call<Unit[]>('unit_list'),
  unitCreate: (name: string) => call<Unit>('unit_create', { name }),
  productList: (query: ProductListQuery) => call<ProductListResult>('product_list', { query }),
  productGet: (id: number) => call<ProductDetail>('product_get', { id }),
  productSave: (input: ProductInput) => call<ProductSaveResult>('product_save', { input }),
  productPricesSave: (input: ProductPricesInput) =>
    call<ProductSaveResult>('product_prices_save', { input }),
  productSetActive: (id: number, active: boolean) => call<void>('product_set_active', { id, active }),
};
