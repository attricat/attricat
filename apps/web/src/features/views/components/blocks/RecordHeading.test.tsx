// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import '../../../../i18n';
import { STATUS_SCHEMA_KEY } from '../../../records/status';
import type { Attribute, ViewDefinition } from '../../../records/api';
import { RecordHeading } from './RecordHeading';

const attributes = [
  { code: 'title', value_type: 'string', context_fallback: 'default' },
] satisfies Attribute[];

const view = {
  type: 'stack',
  component: { id: 'attricat.record_heading', version: 1, props: {} },
  children: [
    { type: 'field', field: 'title' },
    { type: 'text', text: 'Generated catalog product' },
  ],
} satisfies ViewDefinition;

afterEach(cleanup);

describe('RecordHeading', () => {
  it('renders a configured field heading', () => {
    render(
      <RecordHeading
        attributes={attributes}
        recordId="record-1"
        values={{ title: { value: 'Product 001' } }}
        view={view}
      />,
    );
    expect(screen.getByText('Product 001')).toBeTruthy();
    expect(screen.getByText('Generated catalog product')).toBeTruthy();
  });

  it('renders a status field as its chip', () => {
    const status = {
      code: 'state',
      value_type: 'string',
      context_fallback: 'default',
      value_schema: {
        [STATUS_SCHEMA_KEY]: {
          version: 1,
          options: [{ code: 'live', label: 'Live', tone: 'success' }],
        },
      },
    } satisfies Attribute;
    const { container } = render(
      <RecordHeading
        attributes={[...attributes, status]}
        recordId="record-1"
        values={{ title: { value: 'Product 001' }, state: { value: 'live' } }}
        view={{
          ...view,
          children: [...view.children, { type: 'field', field: 'state' }],
        }}
      />,
    );
    expect(screen.getByText('Live')).toBeTruthy();
    expect(container.querySelector('.MuiChip-colorSuccess')).toBeTruthy();
  });
});
