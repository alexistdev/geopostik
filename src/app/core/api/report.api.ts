import type { ExpiryReport } from '../../bindings/ExpiryReport';
import type { ExpiryReportQuery } from '../../bindings/ExpiryReportQuery';
import type { InventoryReport } from '../../bindings/InventoryReport';
import type { ProductReport } from '../../bindings/ProductReport';
import type { PurchaseReport } from '../../bindings/PurchaseReport';
import type { ReportRange } from '../../bindings/ReportRange';
import type { SalesReport } from '../../bindings/SalesReport';
import type { SipnapQuery } from '../../bindings/SipnapQuery';
import type { SipnapReport } from '../../bindings/SipnapReport';
import { call } from './tauri';

export const reportApi = {
  sales: (query: ReportRange) => call<SalesReport>('report_sales', { query }),
  products: (query: ReportRange) => call<ProductReport>('report_products', { query }),
  inventory: () => call<InventoryReport>('report_inventory'),
  expiry: (query: ExpiryReportQuery) => call<ExpiryReport>('report_expiry', { query }),
  purchases: (query: ReportRange) => call<PurchaseReport>('report_purchases', { query }),
  sipnap: (query: SipnapQuery) => call<SipnapReport>('report_sipnap', { query }),
};
