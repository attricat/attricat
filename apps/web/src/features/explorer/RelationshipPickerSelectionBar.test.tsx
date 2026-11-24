// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import type { RecordItem } from '../records/api';
import { RelationshipPickerSelectionBar } from './RelationshipPickerSelectionBar';
import { relationshipPickerManyMessageType } from '../records/components/useRecentlyPreviewedRecords';

const token = '4f0c8e3c-9b0d-4f5a-8f6e-2a1b3c4d5e6f';
const record = (id: string, label: string) =>
  ({ id, display: { default: label } }) as unknown as RecordItem;
const bolt = record('8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f01', 'Bolt');
const nut = record('8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f02', 'Nut');

const renderBar = (
  selectedItems: RecordItem[],
  mode: 'one' | 'many' | 'filter' = 'many',
) =>
  render(
    <RelationshipPickerSelectionBar
      onClear={vi.fn()}
      onRemove={vi.fn()}
      selectedItems={selectedItems}
      session={{ token, mode }}
    />,
  );

describe('RelationshipPickerSelectionBar', () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('sends the selection to the picker that opened the tab and closes it', async () => {
    const postMessage = vi.fn();
    vi.stubGlobal('opener', { closed: false, postMessage });
    const close = vi.spyOn(window, 'close').mockImplementation(() => {});
    renderBar([bolt, nut]);

    await userEvent.click(
      screen.getByRole('button', { name: 'Use 2 selected records' }),
    );

    expect(postMessage).toHaveBeenCalledWith(
      {
        type: relationshipPickerManyMessageType,
        token,
        recordIds: [bolt.id, nut.id],
        labels: { [bolt.id]: 'Bolt', [nut.id]: 'Nut' },
      },
      window.location.origin,
    );
    expect(close).toHaveBeenCalled();
  });

  it('cannot send nothing, or send to a closed record form', () => {
    vi.stubGlobal('opener', { closed: false, postMessage: vi.fn() });
    renderBar([], 'one');
    expect(
      screen.getByText('Select one record in the table, or create a new one.'),
    ).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'Use 0 selected records' }),
    ).toHaveProperty('disabled', true);
    cleanup();

    vi.stubGlobal('opener', null);
    renderBar([bolt]);
    expect(
      screen.getByRole('button', { name: 'Use 1 selected record' }),
    ).toHaveProperty('disabled', true);
    expect(
      screen.getByText(/The record form that opened this tab is closed/),
    ).toBeTruthy();
  });

  it('words the choice for a filter as using the records', async () => {
    vi.stubGlobal('opener', { closed: false, postMessage: vi.fn() });
    await i18n.changeLanguage('pl');
    try {
      renderBar([bolt, nut], 'filter');
      expect(
        screen.getByRole('button', { name: 'Użyj 2 zaznaczonych rekordów' }),
      ).toBeTruthy();
      cleanup();
      renderBar([bolt, nut]);
      expect(
        screen.getByRole('button', { name: 'Dodaj 2 zaznaczone rekordy' }),
      ).toBeTruthy();
    } finally {
      await i18n.changeLanguage('en');
    }
  });
});
