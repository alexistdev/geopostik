import { Injectable } from '@angular/core';

import type { OpnameDetail } from '../../bindings/OpnameDetail';
import { OPNAME_TYPES, formatDate } from './stock-format';

/**
 * Cetak lembar hitung opname lewat dialog print sistem. Stok sistem sengaja tidak dicetak agar
 * penghitung mencatat hasil hitung apa adanya. Isi dirender ke #print-area (lihat styles.scss).
 */
@Injectable({ providedIn: 'root' })
export class CountSheetPrint {
  print(d: OpnameDetail): void {
    document.getElementById('print-area')?.remove();
    const area = document.createElement('div');
    area.id = 'print-area';

    const title = document.createElement('div');
    title.className = 'sheet-title';
    title.textContent = `Lembar Hitung ${OPNAME_TYPES[d.header.opnameType].label} · ${d.header.number}`;
    const meta = document.createElement('div');
    meta.className = 'sheet-meta';
    meta.textContent = [d.header.scopeNote, `Dicetak ${new Date().toLocaleString('id-ID')}`].filter(Boolean).join(' · ');
    area.append(title, meta);

    const table = document.createElement('table');
    table.className = 'count-sheet';
    const head = table.createTHead().insertRow();
    for (const h of ['No', 'Kode', 'Nama obat', 'No. batch', 'ED', 'Satuan', 'Hitung fisik', 'Catatan']) {
      const th = document.createElement('th');
      th.textContent = h;
      head.append(th);
    }
    const body = table.createTBody();
    d.items.forEach((i, n) => {
      const row = body.insertRow();
      for (const v of [String(n + 1), i.productCode, i.productName, i.batchNumber, formatDate(i.expiryDate), i.baseUnitName, '', '']) {
        row.insertCell().textContent = v;
      }
    });
    area.append(table);

    const sign = document.createElement('div');
    sign.className = 'sheet-sign';
    for (const who of ['Dihitung oleh', 'Diperiksa oleh']) {
      const box = document.createElement('div');
      box.textContent = who;
      sign.append(box);
    }
    area.append(sign);
    document.body.append(area);

    const cleanup = () => {
      area.remove();
      window.removeEventListener('afterprint', cleanup);
    };
    window.addEventListener('afterprint', cleanup);
    window.print();
  }
}
