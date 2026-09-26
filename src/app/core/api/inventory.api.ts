import type { OpnameCreateInput } from '../../bindings/OpnameCreateInput';
import type { OpnameDetail } from '../../bindings/OpnameDetail';
import type { OpnameFillInput } from '../../bindings/OpnameFillInput';
import type { OpnameItemInput } from '../../bindings/OpnameItemInput';
import type { OpnamePage } from '../../bindings/OpnamePage';
import type { OpnamePageQuery } from '../../bindings/OpnamePageQuery';
import type { OpnameSaveResult } from '../../bindings/OpnameSaveResult';
import type { ProductBatches } from '../../bindings/ProductBatches';
import type { StockCardPage } from '../../bindings/StockCardPage';
import type { StockCardQuery } from '../../bindings/StockCardQuery';
import type { StockListQuery } from '../../bindings/StockListQuery';
import type { StockPage } from '../../bindings/StockPage';
import { call } from './tauri';

export const inventoryApi = {
  stockList: (query: StockListQuery) => call<StockPage>('stock_list', { query }),
  stockBatches: (productId: number, includeEmpty: boolean) =>
    call<ProductBatches>('stock_batches', { productId, includeEmpty }),
  batchSetLocked: (id: number, locked: boolean, reason: string | null) =>
    call<void>('batch_set_locked', { id, locked, reason }),
  stockCard: (query: StockCardQuery) => call<StockCardPage>('stock_card', { query }),
  opnamePage: (query: OpnamePageQuery) => call<OpnamePage>('opname_page', { query }),
  opnameGet: (id: number) => call<OpnameDetail>('opname_get', { id }),
  opnameCreate: (input: OpnameCreateInput) => call<OpnameDetail>('opname_create', { input }),
  opnameItemSave: (input: OpnameItemInput) => call<OpnameSaveResult>('opname_item_save', { input }),
  opnameItemDelete: (opnameId: number, itemId: number) =>
    call<OpnameDetail>('opname_item_delete', { opnameId, itemId }),
  opnameFill: (input: OpnameFillInput) => call<OpnameSaveResult>('opname_fill', { input }),
  opnameSubmit: (id: number) => call<OpnameDetail>('opname_submit', { id }),
  opnameReopen: (id: number) => call<OpnameDetail>('opname_reopen', { id }),
  opnameCancel: (id: number, reason: string | null) => call<OpnameDetail>('opname_cancel', { id, reason }),
  opnameApprove: (id: number) => call<OpnameDetail>('opname_approve', { id }),
  openingLock: () => call<void>('stock_opening_lock'),
};
