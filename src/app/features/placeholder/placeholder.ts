import { Component, input } from '@angular/core';

/** Halaman sementara untuk menu yang belum dibuat. `title` diisi dari data rute. */
@Component({
  selector: 'app-placeholder',
  template: `
    <h2>{{ title() }}</h2>
    <p>Fitur ini belum dibuat.</p>
  `,
})
export class Placeholder {
  readonly title = input('');
}
