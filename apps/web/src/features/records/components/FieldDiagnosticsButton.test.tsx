// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { Finding } from '../../rules/schemas';
import { RevealFieldContext } from '../../views/fieldReveal';
import {
  FieldDiagnosticsContext,
  fieldFindingsStatuses,
} from '../fieldDiagnostics';
import { FieldDiagnosticsButton } from './FieldDiagnosticsButton';

const polishContextId = crypto.randomUUID();
const metaDescription = {
  code: 'meta_description',
  name: 'Meta description',
  value_type: 'string',
} as const;

const missingDescription: Finding = {
  id: crypto.randomUUID(),
  rule_id: crypto.randomUUID(),
  rule_version: 1,
  record_id: crypto.randomUUID(),
  context_id: polishContextId,
  severity: 'warning',
  message: "'meta_description' is required",
  evidence: { attribute_code: 'meta_description', contexts: ['cars_pl'] },
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '2026-10-20T13:54:17Z',
  updated_at: '2026-10-20T13:54:17Z',
};

describe('FieldDiagnosticsButton', () => {
  it('renders nothing outside the record page', () => {
    const { container } = render(
      <FieldDiagnosticsButton attribute={metaDescription} />,
    );

    expect(container.innerHTML).toBe('');
  });

  it('shows the code, value source and findings of the field', async () => {
    const user = userEvent.setup();
    render(
      <FieldDiagnosticsContext
        value={{
          contextCode: 'cars_pl',
          showsValues: true,
          contextId: polishContextId,
          findings: [missingDescription],
        }}
      >
        <FieldDiagnosticsButton
          attribute={metaDescription}
          resolved={{ value: null }}
        />
      </FieldDiagnosticsContext>,
    );

    await user.click(
      screen.getByRole('button', {
        name: 'Details of Meta description, 1 data quality finding',
      }),
    );

    expect(screen.getByText('meta_description')).toBeTruthy();
    expect(screen.getByText('Empty in this context.')).toBeTruthy();
    expect(screen.getByText('Meta description is required')).toBeTruthy();
  });

  it('says when findings could not be loaded instead of none', async () => {
    const user = userEvent.setup();
    render(
      <FieldDiagnosticsContext
        value={{
          contextCode: 'cars_pl',
          contextId: polishContextId,
          findingsStatus: fieldFindingsStatuses.failed,
          findingsError: new Error('Network down'),
        }}
      >
        <FieldDiagnosticsButton attribute={metaDescription} />
      </FieldDiagnosticsContext>,
    );

    await user.click(
      screen.getByRole('button', { name: 'Details of Meta description' }),
    );

    expect(screen.getByRole('alert').textContent).toBe(
      'Could not load findings: Network down',
    );
    expect(screen.queryByText('No open findings in this context.')).toBeNull();
  });

  it('says findings are loading instead of none', async () => {
    const user = userEvent.setup();
    render(
      <FieldDiagnosticsContext
        value={{
          contextCode: 'cars_pl',
          contextId: polishContextId,
          findingsStatus: fieldFindingsStatuses.loading,
        }}
      >
        <FieldDiagnosticsButton attribute={metaDescription} />
      </FieldDiagnosticsContext>,
    );

    await user.click(
      screen.getByRole('button', { name: 'Details of Meta description' }),
    );

    expect(screen.getByText('Loading findings...')).toBeTruthy();
    expect(screen.queryByText('No open findings in this context.')).toBeNull();
  });

  it('says which context an inherited value comes from', async () => {
    const user = userEvent.setup();
    render(
      <FieldDiagnosticsContext
        value={{
          contextCode: 'cars_pl',
          contextId: polishContextId,
          showsValues: true,
        }}
      >
        <FieldDiagnosticsButton
          attribute={metaDescription}
          resolved={{
            value: 'A fast estate',
            source_context: { id: crypto.randomUUID(), code: 'default' },
          }}
        />
      </FieldDiagnosticsContext>,
    );

    await user.click(
      screen.getByRole('button', { name: 'Details of Meta description' }),
    );

    expect(
      screen.getByText('Inherited from the default context.'),
    ).toBeTruthy();
    // Without access to rules there is no findings section.
    expect(screen.queryByText('Data quality')).toBeNull();
  });

  it('describes how the blueprint ties the field to others', async () => {
    const user = userEvent.setup();
    const attributes = [
      { code: 'sale_status', name: 'Sale status', value_type: 'string' },
      { code: 'main_photo', name: 'Main photo', value_type: 'file' },
      { code: 'description', name: 'Description', value_type: 'string' },
    ] as const;
    render(
      <FieldDiagnosticsContext
        value={{
          attributes,
          connections: [
            {
              kind: 'transition_condition',
              attributeCodes: ['sale_status', 'main_photo', 'description'],
              sources: ['sale_status'],
              name: 'Sale status',
              detail: 'In preparation → Available',
            },
          ],
          contextCode: 'default',
        }}
      >
        <FieldDiagnosticsButton attribute={attributes[1]} />
        <FieldDiagnosticsButton attribute={attributes[0]} />
      </FieldDiagnosticsContext>,
    );

    await user.click(
      screen.getByRole('button', { name: 'Details of Main photo' }),
    );
    expect(
      screen.getByText(
        'Needed to change Sale status: In preparation → Available.',
      ),
    ).toBeTruthy();
    await user.keyboard('{Escape}');

    await user.click(
      screen.getByRole('button', { name: 'Details of Sale status' }),
    );
    expect(
      screen.getByText(
        'In preparation → Available needs Main photo and Description.',
      ),
    ).toBeTruthy();
  });

  it('leads to the fields a connection names', async () => {
    const user = userEvent.setup();
    const revealField = vi.fn();
    const attributes = [
      { code: 'sale_status', name: 'Sale status', value_type: 'string' },
      { code: 'main_photo', name: 'Main photo', value_type: 'file' },
    ] as const;
    render(
      <RevealFieldContext value={revealField}>
        <FieldDiagnosticsContext
          value={{
            attributes,
            connections: [
              {
                kind: 'status_lock',
                attributeCodes: ['sale_status', 'main_photo'],
                sources: ['sale_status'],
                name: 'Sale status',
                detail: 'Sold',
              },
            ],
            contextCode: 'default',
          }}
        >
          <FieldDiagnosticsButton attribute={attributes[1]} />
        </FieldDiagnosticsContext>
      </RevealFieldContext>,
    );

    await user.click(
      screen.getByRole('button', { name: 'Details of Main photo' }),
    );
    expect(screen.getByRole('listitem').textContent).toBe(
      'Locked when Sale status is Sold.',
    );
    await user.click(screen.getByRole('button', { name: 'Sale status' }));

    expect(revealField).toHaveBeenCalledWith('sale_status');
    expect(screen.queryByRole('listitem')).toBeNull();
  });
});
