import { ChangeDetectionStrategy, Component } from '@angular/core';

import { Armazon } from './layout/armazon';

@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [Armazon],
  template: '<app-armazon />',
})
export class App {}
