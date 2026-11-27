// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import { attributeConnectionKinds } from '../attributeConnections';
import type { Attribute } from '../api';
import { FieldConnectionList } from './FieldConnectionList';

const attributes = [
  { code: 'sale_status', name: 'Sale status', value_type: 'string' },
  { code: 'main_photo', name: 'Main photo', value_type: 'file' },
] as Attribute[];

describe('FieldConnectionList', () => {
  it('shows a transition from no status as an empty value', () => {
    render(
      <FieldConnectionList
        attributeCode="main_photo"
        attributes={attributes}
        connections={[
          {
            kind: attributeConnectionKinds.transitionCondition,
            attributeCodes: ['sale_status', 'main_photo'],
            sources: ['sale_status'],
            name: 'Sale status',
            transition: { to: 'Available' },
          },
        ]}
      />,
    );
    expect(screen.getByRole('listitem').textContent).toBe(
      'Needed to change Sale status: —Not set → Available.',
    );
    expect(screen.getByText('—').getAttribute('aria-hidden')).toBe('true');
  });
});
