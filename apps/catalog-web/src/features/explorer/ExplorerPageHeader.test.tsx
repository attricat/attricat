// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ExplorerPageHeader } from './ExplorerPageHeader';

vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({
    children,
    search,
  }: {
    children: ReactNode;
    search: Record<string, unknown>;
  }) => <button data-search={JSON.stringify(search)}>{children}</button>,
}));

const createSearch = () =>
  JSON.parse(screen.getByRole('button').getAttribute('data-search')!);

describe('ExplorerPageHeader', () => {
  it('preselects the explored blueprint in the create form', () => {
    render(
      <ExplorerPageHeader
        blueprint="product"
        locked={false}
        lockedBlueprintName="Product"
      />,
    );
    expect(createSearch()).toEqual({ blueprint: 'product' });
  });

  it('locks the create form to a locked Explorer blueprint', () => {
    render(
      <ExplorerPageHeader
        blueprint="product"
        locked
        lockedBlueprintName="Product"
      />,
    );
    expect(createSearch()).toEqual({ blueprint: 'product', locked: true });
  });

  it('opens a blank create form without a selected blueprint', () => {
    render(
      <ExplorerPageHeader
        blueprint={undefined}
        locked={false}
        lockedBlueprintName={undefined}
      />,
    );
    expect(createSearch()).toEqual({});
  });
});
