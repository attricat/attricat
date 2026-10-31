// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../../i18n';
import type { Finding } from '../../rules/schemas';
import { RevealFieldContext } from '../../views/fieldReveal';
import {
  FieldDiagnosticsContext,
  fieldFindingsStatuses,
} from '../fieldDiagnostics';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import type { ExtensionContribution } from '../../extensions/api';
import {
  attributeEditorStates,
  attributeEditorStatusKey,
  reportAttributeEditorStatus,
} from '../../extensions/native/attributeEditorStatus';
import { nativeUiErrorCodes } from '../../extensions/native/constants';
import { extensionQueryKeys } from '../../extensions/queryKeys';
import { FieldDiagnosticsButton } from './FieldDiagnosticsButton';

vi.mock('../../../components/RouterLink', () => ({
  RouterLink: ({ children }: { children: ReactNode }) => (
    <a href="#extension">{children}</a>
  ),
}));

const editorRecordId = crypto.randomUUID();
const formulaEditor = { id: 'formula', version: 1, props: {} };

/** The record page with the runtime the app loaded. */
const withRuntime = (
  runtime: Partial<ExtensionContribution>[],
  children: ReactNode,
) => {
  const client = new QueryClient();
  client.setQueryData(extensionQueryKeys.runtime(), runtime);
  return (
    <QueryClientProvider client={client}>
      <FieldDiagnosticsContext
        value={{
          contextCode: 'default',
          showsValues: true,
          contextId: undefined,
          findings: [],
        }}
      >
        {children}
      </FieldDiagnosticsContext>
    </QueryClientProvider>
  );
};

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
  afterEach(() => i18n.changeLanguage('en'));

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

  it.each([
    ['en', 'default', 'Inherited from the default context.'],
    ['en', 'cars', 'Inherited from the cars context.'],
    ['pl', 'default', 'Odziedziczona z kontekstu domyślnego.'],
    ['pl', 'cars', 'Odziedziczona z kontekstu cars.'],
  ])(
    'says which context an inherited value comes from (%s, %s)',
    async (language, source, sentence) => {
      await i18n.changeLanguage(language);
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
              source_context: { id: crypto.randomUUID(), code: source },
            }}
          />
        </FieldDiagnosticsContext>,
      );

      await user.click(screen.getByRole('button'));

      expect(screen.getByText(sentence)).toBeTruthy();
      // Without access to rules there is no findings section.
      expect(screen.queryByText('Data quality')).toBeNull();
    },
  );

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

  it('says when the field fell back from an editor whose extension is unavailable', async () => {
    const user = userEvent.setup();
    render(
      withRuntime(
        [],
        <FieldDiagnosticsButton
          attribute={metaDescription}
          editor={formulaEditor}
          recordId={editorRecordId}
          resolved={{ value: null }}
        />,
      ),
    );
    await user.click(
      screen.getByRole('button', { name: 'Details of Meta description' }),
    );
    expect(screen.getByText('Extension editor formula v1')).toBeTruthy();
    expect(
      screen.getByText(/Not available: its extension is disabled/),
    ).toBeTruthy();
  });

  it('names a failed editor, its reason and its extension', async () => {
    const user = userEvent.setup();
    reportAttributeEditorStatus(
      attributeEditorStatusKey(editorRecordId, metaDescription.code),
      {
        state: attributeEditorStates.failed,
        errorCode: nativeUiErrorCodes.unresponsive,
      },
    );
    render(
      withRuntime(
        [
          {
            id: 'formula',
            version: 1,
            kind: 'editor',
            outlet: 'attribute_editor',
            title: 'Formula',
            extension_id: 'acme.formulas',
            extension_name: 'Formulas',
            editor: { value_types: ['string'], props: {} },
          },
        ],
        <FieldDiagnosticsButton
          attribute={metaDescription}
          editor={formulaEditor}
          recordId={editorRecordId}
          resolved={{ value: null }}
        />,
      ),
    );
    await user.click(
      screen.getByRole('button', { name: 'Details of Meta description' }),
    );
    expect(screen.getByText('Formula from Formulas (formula v1)')).toBeTruthy();
    expect(
      screen.getByText(/Formulas stopped responding and was stopped/),
    ).toBeTruthy();
    expect(screen.getByText('Open the extension')).toBeTruthy();
    reportAttributeEditorStatus(
      attributeEditorStatusKey(editorRecordId, metaDescription.code),
    );
  });
});
