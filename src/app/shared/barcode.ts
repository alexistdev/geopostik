import { Component, ElementRef, effect, input, viewChild } from '@angular/core';
import JsBarcode from 'jsbarcode';

/** Format barcode untuk semua label: Code 128 (huruf, angka, tanda hubung). */
export const BARCODE_FORMAT = 'CODE128';

/** Gambar barcode ke elemen SVG. Nilai yang tidak bisa dijadikan barcode menghasilkan SVG kosong. */
export function renderBarcode(svg: SVGElement, value: string, height = 40): void {
  try {
    JsBarcode(svg, value, {
      format: BARCODE_FORMAT,
      height,
      width: 1.6,
      margin: 0,
      fontSize: 12,
      displayValue: true,
    });
  } catch {
    svg.replaceChildren();
  }
}

/** Pratinjau barcode, misal di dialog ubah rak. */
@Component({
  selector: 'app-barcode',
  template: '<svg #svg></svg>',
  styles: ':host { display: inline-block; } svg { max-width: 100%; }',
})
export class Barcode {
  readonly value = input.required<string>();
  readonly height = input(40);
  private readonly svg = viewChild.required<ElementRef<SVGElement>>('svg');

  constructor() {
    effect(() => {
      const value = this.value();
      if (value) {
        renderBarcode(this.svg().nativeElement, value, this.height());
      }
    });
  }
}
