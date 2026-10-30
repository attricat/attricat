// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { DefinitionRevealContext } from '../definition-editor/definitionReveal';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';

describe('BlueprintViewsPreview', () => {
  it('leads from a field to the fields connected to it', async () => {
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <BlueprintViewsPreview
          attributes={[
            { code: 'vin', name: 'VIN', value_type: 'string' },
            { code: 'make', name: 'Make', value_type: 'string' },
          ]}
          definition={`
unique_keys:
  - code: vin_make
    attributes: [vin, make]
`}
          views={{
            detail: {
              type: 'stack',
              children: [
                { type: 'field', field: 'vin' },
                { type: 'field', field: 'make' },
              ],
            },
          }}
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Details of VIN' }));
    await user.click(screen.getByRole('button', { name: 'Make' }));

    await waitFor(() =>
      expect(document.activeElement).toBe(
        screen.getByRole('textbox', { name: 'Make' }),
      ),
    );
  });

  it('leads from a field and a view to where the definition declares them', async () => {
    const user = userEvent.setup();
    const reveal = vi.fn();
    const definition = `attributes:
  - code: vin
    value_type: string
views:
  detail:
    type: stack
    children:
      - {type: field, field: vin}
`;
    render(
      <QueryClientProvider client={new QueryClient()}>
        <DefinitionRevealContext value={reveal}>
          <BlueprintViewsPreview
            attributes={[
              { code: 'vin', name: 'VIN', value_type: 'string' },
              { code: 'make', name: 'Make', value_type: 'string' },
            ]}
            definition={definition}
            views={{
              detail: {
                type: 'stack',
                children: [
                  { type: 'field', field: 'vin' },
                  { type: 'field', field: 'make' },
                ],
              },
            }}
          />
        </DefinitionRevealContext>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Details of VIN' }));
    await user.click(
      screen.getByRole('button', { name: 'Show in definition' }),
    );
    const code = definition.indexOf('vin');
    expect(reveal).toHaveBeenLastCalledWith({
      block: expect.objectContaining({
        start: definition.indexOf('code: vin'),
      }),
      focus: { end: code + 'vin'.length, start: code },
    });

    // A field the edited definition no longer declares has nowhere to lead.
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('button', { name: 'Details of Make' }));
    expect(
      screen.getByRole('button', { name: 'Show in definition' }),
    ).toHaveProperty('disabled', true);
    await user.keyboard('{Escape}');

    await user.click(
      screen.getByRole('button', {
        name: 'Show the detail view in the definition',
      }),
    );
    expect(reveal).toHaveBeenLastCalledWith(
      expect.objectContaining({
        focus: {
          end: definition.indexOf('detail:') + 'detail'.length,
          start: definition.indexOf('detail:'),
        },
      }),
    );
  });
});
