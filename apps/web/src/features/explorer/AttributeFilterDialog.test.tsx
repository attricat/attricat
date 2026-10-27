// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import type { Attribute } from '../records/api';
import { AttributeFilterDialog } from './AttributeFilterDialog';

vi.mock('../principals/usePrincipalDirectory', () => ({
  usePrincipalDirectory: () => ({
    data: undefined,
    error: new ApiRequestError(500, 'Directory failed', 'internal_error'),
  }),
}));

const owner: Attribute = {
  code: 'owner',
  name: 'Owner',
  value_type: 'string',
  value_schema: {
    type: 'string',
    'x-attricat-principal': { version: 1, kinds: ['user', 'team'] },
  },
};

describe('AttributeFilterDialog', () => {
  it('says why only the current user can be chosen when the directory fails', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <AttributeFilterDialog
          attributes={[owner]}
          blueprintName="Task"
          editing={false}
          initialDraft={{ field: 'owner', operator: 'eq', value: '' }}
          maximumReached={false}
          onClose={vi.fn()}
          onSelectRelationship={vi.fn()}
          onSubmit={vi.fn()}
          open
          relationshipPathsLoading={false}
        />
      </QueryClientProvider>,
    );
    expect(screen.getByRole('alert')).toBeTruthy();
  });
});
