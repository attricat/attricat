// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { AttributeContext } from '../../contexts/api';
import { EntityContextPicker } from './EntityContextPicker';

const context = (id: string, code: string) =>
  ({ id, code }) as AttributeContext;
const defaultContext = context('1', 'default');

describe('EntityContextPicker', () => {
  it('stays hidden with only the default context', () => {
    render(
      <EntityContextPicker
        contexts={[defaultContext]}
        onChange={vi.fn()}
        value="1"
      />,
    );
    expect(screen.queryByRole('tablist')).toBeNull();
  });

  it('offers a tab for each context once there are several', () => {
    render(
      <EntityContextPicker
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
