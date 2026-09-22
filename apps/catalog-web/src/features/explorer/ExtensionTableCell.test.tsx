// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from '../extensions/api';
import { ExtensionTableCell } from './ExtensionTableCell';
import { explorerTableCellContextSchema } from './schemas';
import { clearTimingsForTest, recentTimings } from '../inspector/timing';

vi.mock('../extensions/ExtensionFrame', () => ({
  ExtensionFrame: ({
    onFailure,
    onReady,
  }: {
    onFailure?: () => void;
    onReady?: () => void;
  }) => (
    <div>
      <button onClick={onReady}>Renderer ready</button>
      <button onClick={onFailure}>Renderer failed</button>
    </div>
  ),
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
  contribution_key: 'example.extension:example.currency',
  display_order: 0,
  navigation_group: null,
  capabilities: ['client.explorer_table_cell'],
  configuration: null,
  extension_id: 'example.extension',
  extension_name: 'Example Extension',
  id: 'example.currency',
  kind: 'embedded',
  outlet: 'explorer_table_cell',
  release_id: '22222222-2222-4222-8222-222222222222',
  route: null,
  title: null,
  version: 1,
};

describe('ExtensionTableCell', () => {
  afterEach(clearTimingsForTest);

  it('keeps scalar rendering when no enabled compatible renderer is available', () => {
    render(<ExtensionTableCell context={context} fallback="12" frameAllowed />);
    expect(screen.getByText('12')).toBeTruthy();
    expect(screen.queryByText('Renderer frame')).toBeNull();
  });

  it('records only aggregate frame load and fallback durations', () => {
    render(
      <ExtensionTableCell
        context={context}
        contribution={contribution}
        fallback="12"
        frameAllowed
      />,
    );
    const [readyButton] = document.querySelectorAll('button');
    fireEvent.click(readyButton!);
    fireEvent.click(screen.getByRole('button', { name: 'Renderer failed' }));
    expect(recentTimings().map((entry) => entry.phases[0]?.name)).toEqual([
      'frame-fallback',
      'frame-load',
    ]);
    expect(JSON.stringify(recentTimings())).not.toContain('example.extension');
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
