// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from '../extensions/api';
import {
  ExtensionTableCell,
  explorerTableCellContextSchema,
} from './ExtensionTableCell';

vi.mock('../extensions/ExtensionFrame', () => ({
  ExtensionFrame: () => <div>Renderer frame</div>,
}));

const context = explorerTableCellContextSchema.parse({
  context_version: 1,
  column: {
    field: 'price',
    label: 'Price',
    renderer: {
      id: 'example.currency',
      version: 1,
      props: { currency: 'USD' },
    },
  },
  primary_value: 12,
  related_entity: null,
  related_preview: null,
  source_row: {
    entity_id: '11111111-1111-4111-8111-111111111111',
    blueprint_version: 1,
    preview: { default: { price: 12 } },
  },
});

const contribution: ExtensionContribution = {
  capabilities: ['client.explorer_table_cell'],
  configuration: null,
  extension_id: 'example.extension',
  id: 'example.currency',
  kind: 'embedded',
  outlet: 'explorer_table_cell',
  release_id: '22222222-2222-4222-8222-222222222222',
  title: null,
  version: 1,
};

describe('ExtensionTableCell', () => {
  it('keeps scalar rendering when no enabled compatible renderer is available', () => {
    render(<ExtensionTableCell context={context} fallback="12" frameAllowed />);
    expect(screen.getByText('12')).toBeTruthy();
    expect(screen.queryByText('Renderer frame')).toBeNull();
  });

  it('does not exceed the caller-provided virtual-cell frame budget', () => {
    render(
      <ExtensionTableCell
        context={context}
        contribution={contribution}
        fallback="12"
        frameAllowed={false}
      />,
    );
    expect(screen.getByText('12')).toBeTruthy();
    expect(screen.queryByText('Renderer frame')).toBeNull();
  });
});
