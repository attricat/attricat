// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import type { ViewNode } from '../../records/api';
import { FieldDiagnosticsContext } from '../../records/fieldDiagnostics';
import type { Finding } from '../../rules/schemas';
import { ViewTabs } from './ViewTabs';

const field = (code: string) =>
  ({ type: 'field', field: code, component: null }) as ViewNode;
const tabs = [
  { label: 'Overview', children: [field('price')] },
  { label: 'Images', children: [field('photo')] },
];

const contextId = crypto.randomUUID();
const finding = (attributeCode: string): Finding => ({
  id: crypto.randomUUID(),
  rule_id: crypto.randomUUID(),
  rule_version: 1,
  record_id: crypto.randomUUID(),
  context_id: contextId,
  severity: 'warning',
  message: `'${attributeCode}' is required`,
  evidence: { attribute_code: attributeCode },
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '2026-10-20T13:54:17Z',
  updated_at: '2026-10-20T13:54:17Z',
});

const description = (name: string) =>
  screen.getByRole('tab', { name }).getAttribute('aria-description');

describe('ViewTabs', () => {
  it('marks tabs that hold a field with an error', () => {
    render(
      <ViewTabs
        invalidFields={new Set(['photo'])}
        render={() => null}
        tabs={tabs}
      />,
    );
    expect(
      screen
        .getByRole('tab', { name: 'Overview' })
        .getAttribute('aria-description'),
    ).toBeNull();
    expect(
      screen
        .getByRole('tab', { name: 'Images' })
        .getAttribute('aria-description'),
    ).toBe('Contains fields that need attention');
  });

  it('marks tabs that hold a field with data quality findings', () => {
    render(
      <FieldDiagnosticsContext
        value={{ contextId, findings: [finding('photo')] }}
      >
        <ViewTabs render={() => null} tabs={tabs} />
      </FieldDiagnosticsContext>,
    );
    expect(description('Overview')).toBeNull();
    expect(description('Images')).toBe(
      'Contains fields with data quality findings',
    );
  });

  it('ignores findings about other contexts', () => {
    render(
      <FieldDiagnosticsContext
        value={{
          contextId: crypto.randomUUID(),
          findings: [finding('photo')],
        }}
      >
        <ViewTabs render={() => null} tabs={tabs} />
      </FieldDiagnosticsContext>,
    );
    expect(description('Images')).toBeNull();
  });

  it('marks errors over findings', () => {
    render(
      <FieldDiagnosticsContext
        value={{ contextId, findings: [finding('photo')] }}
      >
        <ViewTabs
          invalidFields={new Set(['photo'])}
          render={() => null}
          tabs={tabs}
        />
      </FieldDiagnosticsContext>,
    );
    expect(description('Images')).toBe('Contains fields that need attention');
  });
});
