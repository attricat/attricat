// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { AttributeContext } from '../../contexts/api';
import { RecordContextPicker } from './RecordContextPicker';

const context = (id: string, code: string) =>
  ({ id, code }) as AttributeContext;
const defaultContext = context('1', 'default');

describe('RecordContextPicker', () => {
  it('stays hidden with only the default context', () => {
    render(
      <RecordContextPicker
        contexts={[defaultContext]}
        onChange={vi.fn()}
        value="1"
      />,
    );
    expect(screen.queryByRole('tablist')).toBeNull();
  });

  it('offers a tab for each context once there are several', () => {
    render(
      <RecordContextPicker
        contexts={[defaultContext, context('2', 'storefront')]}
        onChange={vi.fn()}
        value="1"
      />,
    );
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'Default',
      'storefront',
    ]);
  });
});
