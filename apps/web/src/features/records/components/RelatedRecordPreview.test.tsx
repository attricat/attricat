// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { AttributeValue } from '../../views/components/values/AttributeValue';
import {
  getBlueprintRevision,
  getRecordLabels,
  getResolvedRecordPreview,
  type Attribute,
} from '../api';
import {
  RelatedRecordPreviewContext,
  singleLinkedRecord,
} from '../linkedRecord';
import { RelationshipField } from './RelationshipField';

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  Link: ({
    children,
    rel,
    target,
  }: {
    children: ReactNode;
    rel?: string;
    target?: string;
  }) => (
    <a href="/records" rel={rel} target={target}>
      {children}
    </a>
  ),
}));
vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  getBlueprintRevision: vi.fn(),
  getRecordLabels: vi.fn(),
  listRecordBlueprints: vi.fn(),
  getResolvedRecordPreview: vi.fn(),
}));

const productId = '123e4567-e89b-12d3-a456-426614174010';
const otherId = '123e4567-e89b-12d3-a456-426614174011';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const details = 'Details of Red shirt';

const relationship = (cardinality: 'one' | 'many') =>
  ({
    code: 'product',
    value_type: 'relationship',
    cardinality,
    target_blueprint_code: 'product',
  }) as Attribute;

const linked = { items: [{ id: productId, display: 'Red shirt' }] };

const renderInContext = (children: ReactNode, previewContextId = contextId) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RelatedRecordPreviewContext.Provider value={previewContextId}>
        {children}
      </RelatedRecordPreviewContext.Provider>
    </QueryClientProvider>,
  );

const mockLinkedRecord = () => {
  vi.mocked(getResolvedRecordPreview).mockResolvedValue({
    record: { id: productId, blueprint_id: blueprintId, blueprint_version: 2 },
    values: {
      color: { value: 'Red', source_context: { id: contextId, code: 'en' } },
      // The linked record's own single relationship.
      brand: {
        value: { items: [{ id: otherId, display: 'Acme' }] },
        source_context: { id: contextId, code: 'en' },
      },
    },
  } as never);
  vi.mocked(getBlueprintRevision).mockResolvedValue({
    blueprint: { views: {} },
    attributes: [
      { code: 'color', value_type: 'string', label: 'Color' },
      {
        code: 'brand',
        value_type: 'relationship',
        cardinality: 'one',
        label: 'Brand',
      },
    ],
  } as never);
};

describe('singleLinkedRecord', () => {
  it('returns the record a single relationship links to', () => {
    expect(singleLinkedRecord(relationship('one'), linked)).toEqual(
      linked.items[0],
    );
  });

  it('returns nothing for a list, an empty value or a scalar', () => {
    expect(singleLinkedRecord(relationship('many'), linked)).toBeUndefined();
    expect(
      singleLinkedRecord(relationship('one'), { items: [] }),
    ).toBeUndefined();
    expect(
      singleLinkedRecord(relationship('one'), {
        items: [{ id: productId }, { id: otherId }],
      }),
    ).toBeUndefined();
    expect(
      singleLinkedRecord(
        { code: 'name', value_type: 'string' } as Attribute,
        'Red shirt',
      ),
    ).toBeUndefined();
  });
});

describe('a single relationship value on the record page', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('opens the linked record on request, in the same context', async () => {
    mockLinkedRecord();
    renderInContext(
      <AttributeValue attribute={relationship('one')} value={linked} />,
    );

    // Clicks synchronously, so the record is still loading when it opens.
    fireEvent.click(screen.getByRole('button', { name: details }));
    const region = screen.getByRole('region', { name: details });
    expect(
      within(region)
        .getByLabelText('Loading the linked record')
        .getAttribute('aria-busy'),
    ).toBe('true');
    expect(await within(region).findByText('Red')).toBeTruthy();
    expect(
      within(region).queryByLabelText('Loading the linked record'),
    ).toBeNull();
    expect(getResolvedRecordPreview).toHaveBeenCalledWith(productId, contextId);
    expect(getBlueprintRevision).toHaveBeenCalledWith(blueprintId, 2);
    // The linked record's own relationship is only a link, labelled as one.
    expect(within(region).getByText('Acme')).toBeTruthy();
    expect(
      within(region).getByText('brand').parentElement?.querySelector('svg'),
    ).toBeTruthy();
    expect(
      within(region).getByText('color').parentElement?.querySelector('svg'),
    ).toBeNull();
    expect(screen.getAllByRole('button', { name: /^Details of/ })).toHaveLength(
      1,
    );
  });

  it('opens the linked record in a new tab', () => {
    mockLinkedRecord();
    renderInContext(
      <AttributeValue attribute={relationship('one')} value={linked} />,
    );

    const link = screen.getByText('Red shirt').closest('a');
    expect(link?.getAttribute('target')).toBe('_blank');
    expect(link?.getAttribute('rel')).toBe('noopener');
    expect(
      screen.getByRole('link', { name: /Opens in a new tab.*Red shirt/ }),
    ).toBe(link);
  });

  it('shows the linked record closed until opened from beside its link', async () => {
    mockLinkedRecord();
    renderInContext(
      <AttributeValue attribute={relationship('one')} value={linked} />,
    );

    const toggle = screen.getByRole('button', { name: details });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('region', { name: details })).toBeNull();
    expect(getResolvedRecordPreview).not.toHaveBeenCalled();
    await userEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    await userEvent.click(toggle);

    expect(toggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('says when the linked record cannot be shown', async () => {
    vi.mocked(getResolvedRecordPreview).mockRejectedValue(
      new Error('forbidden'),
    );
    renderInContext(
      <AttributeValue attribute={relationship('one')} value={linked} />,
    );

    await userEvent.click(screen.getByRole('button', { name: details }));
    expect(
      await screen.findByText('This record cannot be shown.'),
    ).toBeTruthy();
  });

  it('shows only links for a list, or outside the record page', () => {
    renderInContext(
      <AttributeValue attribute={relationship('many')} value={linked} />,
    );
    expect(screen.queryByRole('button', { name: details })).toBeNull();

    renderInContext(
      <AttributeValue attribute={relationship('one')} value={linked} />,
      null as never,
    );
    expect(screen.queryByRole('button', { name: details })).toBeNull();
    expect(getResolvedRecordPreview).not.toHaveBeenCalled();
  });
});

describe('a single relationship field on the record page', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each([
    ['one target blueprint', relationship('one')],
    [
      'no target blueprint',
      {
        ...relationship('one'),
        target_blueprint_code: null,
        target_blueprint_codes: [],
      },
    ],
  ])('shows the linked record within a field with %s', async (_, attribute) => {
    mockLinkedRecord();
    vi.mocked(getRecordLabels).mockRejectedValue(new Error('offline'));
    renderInContext(
      <RelationshipField
        attribute={{ ...attribute, name: 'Product' }}
        onChange={vi.fn()}
        value={productId}
      />,
    );

    const field = screen.getByRole('group', { name: 'Product' });
    await userEvent.click(
      within(field).getByRole('button', { name: `Details of ${productId}` }),
    );
    const region = within(field).getByRole('region', {
      name: `Details of ${productId}`,
    });
    expect(await within(region).findByText('Red')).toBeTruthy();
  });
});
