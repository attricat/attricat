// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ExplorerSearchForm } from './ExplorerSearchForm';

const revisions = [
  {
    code: 'product',
    name: 'Product',
    version: 3,
    status: 'published' as const,
    views: {},
  },
  {
    code: 'product',
    name: 'Product',
    version: 2,
    status: 'published' as const,
    views: {},
  },
];

describe('ExplorerSearchForm version scope', () => {
  it('shows the blueprint selector when the route is not locked', () => {
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{}}
      />,
    );

    expect(
      screen.getByRole('combobox', { name: /select a blueprint/i }),
    ).toBeTruthy();
  });

  it('defaults to current and searches immediately for a historical version', async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={onSubmit}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );

    expect(screen.getByLabelText('Version scope').textContent).toContain(
      'Current — v3',
    );
    await user.click(screen.getByLabelText('Version scope'));
    await user.click(screen.getByRole('option', { name: 'Version 2' }));

    expect(onSubmit).toHaveBeenCalledOnce();
    expect(onSubmit).toHaveBeenCalledWith({
      blueprint: 'product',
      version: 2,
      query: undefined,
    });
  });

  it('submits all versions as the explicit all-version scope', async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={onSubmit}
        revisions={revisions}
        search={{ blueprint: 'product', allVersions: true }}
      />,
    );

    await user.click(screen.getByRole('button', { name: 'Search' }));
    expect(onSubmit).toHaveBeenCalledWith({
      blueprint: 'product',
      allVersions: true,
      query: undefined,
    });
  });
});
