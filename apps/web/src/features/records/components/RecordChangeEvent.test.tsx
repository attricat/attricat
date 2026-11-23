// @vitest-environment jsdom
import {
  cleanup,
  render as renderElement,
  screen,
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement } from 'react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { RecordAuditChange } from '../api';
import { RecordChangeEvent } from './RecordChangeEvent';

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  Link: ({
    children,
    params,
    rel,
    target,
  }: {
    children: ReactNode;
    params: { recordId: string };
    rel?: string;
    target?: string;
  }) => (
    <a href={`/records/${params.recordId}`} rel={rel} target={target}>
      {children}
    </a>
  ),
}));

const brandId = '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f01';
const formerId = '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f02';

const change = (overrides: Partial<RecordAuditChange>): RecordAuditChange => ({
  audit_event_id: '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f10',
  occurred_at: '2026-11-23T10:00:00Z',
  actor_user_id: null,
  actor_display_name: 'Ada',
  actor_email: null,
  actor_avatar_file_id: null,
  executor_type: 'user',
  agent_run_id: null,
  approval_decision: null,
  approved_by_user_id: null,
  approved_by_display_name: null,
  approved_by_avatar_file_id: null,
  attribute_id: '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f20',
  attribute_code: 'brand',
  context_id: null,
  context_code: null,
  change_kind: 'relationship_add',
  deleted_file_ids: [],
  before_value: null,
  after_value: brandId,
  ...overrides,
});

// User labels look people up.
const render = (element: ReactElement) =>
  renderElement(
    <QueryClientProvider client={new QueryClient()}>
      {element}
    </QueryClientProvider>,
  );

describe('RecordChangeEvent', () => {
  afterEach(cleanup);

  it('links a linked or unlinked record, opening it in a new tab', () => {
    render(
      <RecordChangeEvent
        changes={[
          change({}),
          change({
            change_kind: 'relationship_remove',
            before_value: formerId,
            after_value: null,
          }),
        ]}
        recordLabels={new Map([[brandId, 'Acme']])}
      />,
    );
    const added = screen.getByRole('link', { name: 'Opens in a new tab Acme' });
    expect(added.getAttribute('href')).toBe(`/records/${brandId}`);
    expect(added.getAttribute('target')).toBe('_blank');
    expect(added.getAttribute('rel')).toBe('noopener');
    // A record without a known label is named by its ID.
    expect(
      screen
        .getByRole('link', { name: `Opens in a new tab ${formerId}` })
        .getAttribute('href'),
    ).toBe(`/records/${formerId}`);
  });

  it('keeps other changes as values', () => {
    render(
      <RecordChangeEvent
        changes={[
          change({
            attribute_code: 'title',
            change_kind: 'set',
            before_value: 'Old',
            after_value: 'New',
          }),
        ]}
      />,
    );
    expect(screen.queryByRole('link')).toBeNull();
    expect(screen.getByText('"New"')).toBeTruthy();
  });
});
