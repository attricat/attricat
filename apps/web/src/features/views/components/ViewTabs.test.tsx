// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import type { ViewNode } from '../../records/api';
import { ViewTabs } from './ViewTabs';

const field = (code: string) =>
  ({ type: 'field', field: code, component: null }) as ViewNode;
const tabs = [
  { label: 'Overview', children: [field('price')] },
  { label: 'Images', children: [field('photo')] },
];

describe('ViewTabs', () => {
  it('marks tabs that hold a field with an error', () => {
    render(
      <ViewTabs
        invalidFields={new Set(['photo'])}
        render={() => null}
        tabs={tabs}
      />,
    );
    expect(
      screen
        .getByRole('tab', { name: 'Overview' })
        .getAttribute('aria-description'),
    ).toBeNull();
    expect(
      screen
        .getByRole('tab', { name: 'Images' })
        .getAttribute('aria-description'),
    ).toBe('Contains fields that need attention');
  });
});
