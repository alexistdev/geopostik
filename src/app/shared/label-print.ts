import { Injectable } from '@angular/core';

import { renderBarcode } from './barcode';

export interface LabelItem {
  code: string;
  name: string;
}

/**
 * Cetak label barcode lewat dialog print sistem (printer label atau kertas A4 berisi grid label).
 * Isi label dirender ke #print-area; saat mencetak, bagian lain halaman disembunyikan (styles.scss).
 */
@Injectable({ providedIn: 'root' })
export class LabelPrint {
  print(title: string, items: LabelItem[]): void {
    if (!items.length) return;

    document.getElementById('print-area')?.remove();
    const area = document.createElement('div');
    area.id = 'print-area';

    const heading = document.createElement('div');
    heading.className = 'label-title';
    heading.textContent = title;
    area.append(heading);

    const sheet = document.createElement('div');
    sheet.className = 'label-sheet';
    for (const item of items) {
      const label = document.createElement('div');
      label.className = 'label';

      const name = document.createElement('div');
      name.className = 'label-name';
      name.textContent = item.name;

      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      renderBarcode(svg, item.code, 36);

      label.append(name, svg);
      sheet.append(label);
    }
    area.append(sheet);
    document.body.append(area);

    const cleanup = () => {
      area.remove();
      window.removeEventListener('afterprint', cleanup);
    };
    window.addEventListener('afterprint', cleanup);
    window.print();
  }
}
