// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import { LEXICON_MANAGEMENT_NAMESPACE } from './constants';
import { currentSession } from '../auth/api';
import {
  deleteLexiconEntry,
  getLexiconReport,
  listStoredLexiconEntries,
  saveLexiconEntry,
} from './api';
import { LexiconPage } from './LexiconPage';
import type { StoredLexiconEntry } from './schemas';

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  deleteLexiconEntry: vi.fn(),
  exportLexicon: vi.fn(),
  getLexiconReport: vi.fn(),
  importLexicon: vi.fn(),
  listStoredLexiconEntries: vi.fn(),
  saveLexiconEntry: vi.fn(),
}));
vi.mock('./lexicon', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./lexicon')>()),
  reloadLexicon: vi.fn(),
}));

const entry = (
  key: string,
  text: string,
  overrides: Partial<StoredLexiconEntry> = {},
): StoredLexiconEntry => ({
  key,
  context: null,
  language: 'pl',
  plural_category: 'other',
  text,
  source: 'workspace',
  solution_pack_id: null,
  updated_at: '2026-10-02T09:00:00Z',
  ...overrides,
});

const session = (blueprintsWrite: boolean) =>
  ({
    capabilities: { blueprints_write: blueprintsWrite },
  }) as Awaited<ReturnType<typeof currentSession>>;

const renderPage = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <LexiconPage language="pl" onLanguageChange={vi.fn()} />
    </QueryClientProvider>,
  );

describe('LexiconPage', () => {
  beforeAll(() => i18n.loadNamespaces(LEXICON_MANAGEMENT_NAMESPACE));

  beforeEach(() => {
    vi.mocked(currentSession).mockResolvedValue(session(true));
    vi.mocked(listStoredLexiconEntries).mockResolvedValue([
      entry('Name', 'Nazwa'),
      entry('Price', 'Cena', {
        source: 'solution_pack',
        solution_pack_id: 'acme.shop',
      }),
      entry('Removed', 'Usunięte'),
      entry('Name', 'Name', { language: 'en' }),
    ]);
    vi.mocked(getLexiconReport).mockResolvedValue({
      reference_count: 3,
      languages: [
        {
          language: 'pl',
          translated_count: 2,
          untranslated: [{ key: 'Overview', context: null }],
          missing_plural_categories: [
            { key: 'Product', context: null, missing: ['few', 'many'] },
          ],
        },
      ],
      orphaned: [{ key: 'Removed', context: null, languages: ['pl'] }],
    });
    vi.mocked(saveLexiconEntry).mockImplementation(async (saved) =>
      entry(saved.key, saved.text, saved),
    );
    vi.mocked(deleteLexiconEntry).mockResolvedValue();
  });

  it('lists the language entries, their sources, and unused keys', async () => {
    renderPage();
    const table = await screen.findByRole('table');
    expect(within(table).getAllByRole('row')).toHaveLength(4);
    expect(within(table).getByText('Nazwa')).toBeTruthy();
    expect(within(table).getByText('acme.shop')).toBeTruthy();
    expect(within(table).getByText('Unused')).toBeTruthy();
    expect(screen.getByText('2 labels need translation')).toBeTruthy();

    await userEvent.type(screen.getByLabelText('Filter translations'), 'cena');
    expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(
      2,
    );
  });

  it('translates a missing label from the coverage list', async () => {
    renderPage();
    await userEvent.click(
      await screen.findByRole('button', { name: 'Translate' }),
    );
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByLabelText(/^Key/)).toHaveProperty(
      'value',
      'Overview',
    );
    await userEvent.type(
      within(dialog).getByLabelText(/^Translation/),
      'Przegląd',
    );
    await userEvent.click(
      within(dialog).getByRole('button', { name: 'Save translation' }),
    );
    await waitFor(() =>
      expect(saveLexiconEntry).toHaveBeenCalledWith(
        {
          key: 'Overview',
          context: null,
          language: 'pl',
          plural_category: 'other',
          text: 'Przegląd',
        },
        expect.anything(),
      ),
    );
  });

  it('adds a missing plural form with the category preselected', async () => {
    renderPage();
    await userEvent.click(
      await screen.findByRole('button', { name: 'Add few' }),
    );
    const dialog = await screen.findByRole('dialog');
    await userEvent.type(
      within(dialog).getByLabelText(/^Translation/),
      'Produkty',
    );
    await userEvent.click(
      within(dialog).getByRole('button', { name: 'Save translation' }),
    );
    await waitFor(() =>
      expect(vi.mocked(saveLexiconEntry).mock.calls[0][0]).toMatchObject({
        key: 'Product',
        plural_category: 'few',
      }),
    );
  });

  it('deletes an entry after confirmation', async () => {
    renderPage();
    await userEvent.click(
      await screen.findByRole('button', { name: 'Delete translation of Name' }),
    );
    const dialog = await screen.findByRole('dialog');
    await userEvent.click(
      within(dialog).getByRole('button', { name: 'Delete' }),
    );
    await waitFor(() =>
      expect(vi.mocked(deleteLexiconEntry).mock.calls[0][0]).toMatchObject({
        key: 'Name',
        language: 'pl',
      }),
    );
  });

  it('hides every editing action without blueprint write access', async () => {
    vi.mocked(currentSession).mockResolvedValue(session(false));
    renderPage();
    expect(
      await screen.findByText(/Changing them requires permission/),
    ).toBeTruthy();
    await screen.findByRole('table');
    for (const name of [
      'Add translation',
      'Import',
      'Translate',
      'Add few',
      'Delete translation of Name',
    ])
      expect(screen.queryByRole('button', { name })).toBeNull();
    expect(screen.getByRole('button', { name: 'Export' })).toBeTruthy();
  });
});
