// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { ApiRequestError } from '../../../api/request';
import { duplicateRecord } from '../api';
import { DUPLICATE_NEEDS_UNIQUE_VALUES_ERROR_CODE } from '../constants';
import { DuplicateRecordDialog } from './DuplicateRecordDialog';

const { navigate } = vi.hoisted(() => ({ navigate: vi.fn() }));
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => navigate }));
vi.mock('../api', () => ({ duplicateRecord: vi.fn() }));

describe('DuplicateRecordDialog', () => {
  it('offers creating another record when the copy would lack a unique value', async () => {
    vi.mocked(duplicateRecord).mockRejectedValue(
      new ApiRequestError(
        422,
        'copy needs unique values',
        DUPLICATE_NEEDS_UNIQUE_VALUES_ERROR_CODE,
        { attributes: ['code'] },
      ),
    );
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <DuplicateRecordDialog
          blueprint={{ code: 'equipment', name: 'Equipment' }}
          onClose={onClose}
          onDuplicated={vi.fn()}
          recordId="00000000-0000-4000-8000-000000000001"
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Duplicate record' }));
    await user.click(
      await screen.findByRole('button', { name: 'Create another Equipment' }),
    );

    expect(duplicateRecord).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith({
      search: { blueprint: 'equipment' },
      to: '/records/new',
    });
  });
});
