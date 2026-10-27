// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import { LEXICON_NAMESPACE } from '../lexicon/constants';
import { getSystemTagFacet } from '../records/api';
import { SystemTagFilterPicker } from './SystemTagFilterPicker';

vi.mock('../records/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../records/api')>();
  return { ...actual, getSystemTagFacet: vi.fn() };
});

const facet = vi.mocked(getSystemTagFacet);

const openPicker = async (onAdd = vi.fn()) => {
  const user = userEvent.setup();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <SystemTagFilterPicker blueprint="product" onAdd={onAdd} selected={[]} />
    </QueryClientProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Tag' }));
  await screen.findAllByRole('option');
  return user;
};

const optionNames = () =>
  screen.getAllByRole('option').map((option) => option.textContent);

describe('SystemTagFilterPicker', () => {
  beforeEach(() => {
    facet.mockReset();
  });

  afterEach(() => {
    i18n.removeResourceBundle('en', LEXICON_NAMESPACE);
  });

  it('shows translated tags and matches typing against either form', async () => {
    i18n.addResourceBundle('en', LEXICON_NAMESPACE, {
      'workflow.review': 'Needs review',
    });
    facet.mockResolvedValue({
      items: [
        { tag: 'workflow.review', count: 5 },
        { tag: 'imported', count: 3 },
      ],
      truncated: false,
    });
    const onAdd = vi.fn();
    const user = await openPicker(onAdd);

    expect(optionNames()).toEqual([
      'Needs review5 records',
      'imported3 records',
    ]);
    await user.type(screen.getByRole('combobox'), 'needs');
    expect(optionNames()).toEqual(['Needs review5 records']);
    await user.clear(screen.getByRole('combobox'));
    await user.type(screen.getByRole('combobox'), 'workflow.');
    expect(optionNames()).toEqual(['Needs review5 records']);

    await user.click(screen.getByRole('option'));
    expect(onAdd).toHaveBeenCalledWith('workflow.review');
  });

  it('filters a complete list in the browser without asking the server again', async () => {
    facet.mockResolvedValue({
      items: [
        { tag: 'needs-review', count: 12 },
        { tag: 'imported', count: 3 },
        { tag: 'Needs-photos', count: 1 },
      ],
      truncated: false,
    });
    const user = await openPicker();

    await user.type(screen.getByRole('combobox'), 'NEED');

    expect(optionNames()).toEqual([
      'needs-review12 records',
      'Needs-photos1 record',
    ]);
    // Wait past the debounce: the typed prefix never reaches the server.
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(facet).toHaveBeenCalledTimes(1);
    expect(facet).toHaveBeenCalledWith(
      { blueprint: { code: 'product' }, limit: 50 },
      expect.anything(),
    );
  });

  it('asks the server for a prefix when the first list was truncated', async () => {
    facet.mockImplementation(async (request) =>
      request.prefix
        ? { items: [{ tag: 'rare-tag', count: 1 }], truncated: false }
        : { items: [{ tag: 'common', count: 40 }], truncated: true },
    );
    const user = await openPicker();

    await user.type(screen.getByRole('combobox'), 'rare');

    await waitFor(() => expect(optionNames()).toEqual(['rare-tag1 record']));
    expect(facet).toHaveBeenLastCalledWith(
      { blueprint: { code: 'product' }, prefix: 'rare', limit: 50 },
      expect.anything(),
    );
  });

  it('reuses a complete answer for a shorter prefix as the prefix grows', async () => {
    facet.mockImplementation(async (request) =>
      request.prefix
        ? {
            items: [
              { tag: 'rare-tag', count: 2 },
              { tag: 'raw-import', count: 1 },
            ],
            truncated: false,
          }
        : { items: [{ tag: 'common', count: 40 }], truncated: true },
    );
    const user = await openPicker();
    const combobox = screen.getByRole('combobox');

    await user.type(combobox, 'ra');
    await waitFor(() =>
      expect(optionNames()).toEqual([
        'rare-tag2 records',
        'raw-import1 record',
      ]),
    );
    await user.type(combobox, 'r');
    expect(optionNames()).toEqual(['rare-tag2 records']);
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(facet.mock.calls.map(([request]) => request.prefix)).toEqual([
      undefined,
      'ra',
    ]);
  });
});
