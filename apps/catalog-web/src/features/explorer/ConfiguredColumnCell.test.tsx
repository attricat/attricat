// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import type { EntityItem } from '../entities/api';
import { ConfiguredColumnCell } from './ConfiguredColumnCell';

const entity: EntityItem = {
  id: '123e4567-e89b-12d3-a456-426614174001',
  blueprint_version: 1,
  schema_outdated: false,
  is_sample: false,
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
        renderer: { id: 'catalog.email_display', version, props: {} },
        relationshipSortBlocked: false,
        sortable: false,
      }}
      entity={{ ...entity, table_values: { [field]: values } }}
      extension={undefined}
      frameAllowed={false}
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
