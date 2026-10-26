// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import '../../i18n';
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
[[unique_keys]]
code = "vin_make"
attributes = ["vin", "make"]
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
});
