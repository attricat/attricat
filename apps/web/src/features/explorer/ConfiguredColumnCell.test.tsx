// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import type { ExtensionContribution } from '../extensions/api';
import type { RecordItem } from '../records/api';
import { ConfiguredColumnCell } from './ConfiguredColumnCell';

const record: RecordItem = {
  id: '123e4567-e89b-12d3-a456-426614174001',
  blueprint_version: 1,
  schema_outdated: false,
  is_sample: false,
  system_tags: [],
  display: {},
  preview: {},
  table_values: {},
  match_explanations: [],
};

const renderCell = (field: string, values: unknown[], version = 1) =>
  render(
    <ConfiguredColumnCell
      attribute={{ code: field, value_type: 'string' }}
      column={{
        field,
        renderer: { id: 'attricat.email_display', version, props: {} },
        relationshipSortBlocked: false,
        sortable: false,
      }}
      record={{ ...record, table_values: { [field]: values } }}
      extension={undefined}
    />,
  );

describe('configured email columns', () => {
  it('renders local email values without enabling extension frames', () => {
    renderCell('email', ['a@example.test']);
    expect(screen.getByRole('link').getAttribute('href')).toBe(
      'mailto:a@example.test',
    );
  });
  it('handles multiple projected values and invalid legacy values', () => {
    renderCell('contacts.email', [
      'a@example.test',
      'b@example.test',
      '<b>invalid</b>',
    ]);
    expect(screen.getAllByRole('link')).toHaveLength(2);
    expect(screen.getByText('<b>invalid</b>').closest('a')).toBeNull();
  });
  it('shows absent relationship values without fetching', () => {
    renderCell('contacts.email', []);
    expect(screen.getByText('Not set')).toBeTruthy();
    expect(screen.queryByRole('link')).toBeNull();
  });
  it('does not resolve an unknown renderer version', () => {
    renderCell('email', ['a@example.test'], 2);
    expect(screen.queryByRole('link')).toBeNull();
    expect(screen.getByText('a@example.test')).toBeTruthy();
  });
});

describe('declared extension cells', () => {
  const extension: ExtensionContribution = {
    contribution_key: 'acme.money:amount',
    display_order: 0,
    navigation_group: null,
    extension_id: 'acme.money',
    extension_name: 'Acme Money',
    release_id: '123e4567-e89b-12d3-a456-426614174000',
    configuration: {},
    capabilities: ['client.explorer_table_cell'],
    id: 'acme.money.amount',
    version: 1,
    kind: 'cell',
    outlet: 'explorer_table_cell',
    route: null,
    title: null,
    invoke: null,
    when: null,
    confirm: null,
    accepts: [],
    icon: null,
    translations: {},
    editor: null,
    rendering: null,
    ui_kit: null,
    artifact_revision: null,
    cell: {
      value_types: ['number'],
      props: {},
      display: {
        type: 'number',
        precision: { prop: 'precision' },
        unit: { prop: 'unit' },
      },
    },
  };
  const renderAmount = (values: unknown[], props: Record<string, unknown>) =>
    render(
      <ConfiguredColumnCell
        attribute={{ code: 'price', value_type: 'number' }}
        column={{
          field: 'price',
          renderer: { id: 'acme.money.amount', version: 1, props },
          relationshipSortBlocked: false,
          sortable: false,
        }}
        record={{ ...record, table_values: { price: values } }}
        extension={extension}
      />,
    );

  it('draws each value as the cell declares, with the column props', () => {
    renderAmount([55], { precision: 2, unit: 'EUR' });
    expect(screen.getByText('55.00 EUR')).toBeTruthy();
  });

  it('ignores a precision outside the bounds and shows several values', () => {
    renderAmount([1.5, 2], { precision: 99 });
    expect(screen.getByText('1.5, 2')).toBeTruthy();
  });

  it('keeps the host rendering for a value it cannot show', () => {
    renderAmount(['n/a'], { precision: 2 });
    expect(screen.getByText('n/a')).toBeTruthy();
  });
});
