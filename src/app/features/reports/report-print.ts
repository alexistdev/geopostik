import { Injectable } from '@angular/core';

/**
 * Cetak isi laporan lewat dialog print sistem (bisa "Simpan sebagai PDF"). Elemen laporan disalin
 * ke #print-area (lihat styles.scss); elemen bertanda `.no-print` tidak ikut dicetak.
 */
@Injectable({ providedIn: 'root' })
export class ReportPrint {
  print(title: string, subtitle: string, content: HTMLElement): void {
    document.getElementById('print-area')?.remove();
    const area = document.createElement('div');
    area.id = 'print-area';
    area.className = 'report-print';

    const head = document.createElement('div');
    head.className = 'sheet-title';
    head.textContent = title;
    const meta = document.createElement('div');
    meta.className = 'sheet-meta';
    meta.textContent = [subtitle, `Dicetak ${new Date().toLocaleString('id-ID')}`].filter(Boolean).join(' · ');

    const body = content.cloneNode(true) as HTMLElement;
    body.querySelectorAll('.no-print').forEach((el) => el.remove());
    area.append(head, meta, body);
    document.body.append(area);

    const cleanup = () => {
      area.remove();
      window.removeEventListener('afterprint', cleanup);
    };
    window.addEventListener('afterprint', cleanup);
    window.print();
  }
}
