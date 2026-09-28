// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { DraftProposal } from './DraftProposal';

describe('DraftProposal', () => {
  it('applies only selected fields and skips values edited since the proposal', async () => {
    const onApply = vi.fn();
    const user = userEvent.setup();
    render(
      <DraftProposal
        proposal={{
          fields: { title: 'New title', description: 'New description' },
          explanation: 'From the source',
          baseValues: { title: 'Old title', description: 'Old description' },
        }}
        getDraftValues={() => ({
          title: 'Changed by user',
          description: 'Old description',
        })}
        onApply={onApply}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Apply to form' }));
    expect(onApply).toHaveBeenCalledWith({ description: 'New description' });
    expect(screen.getByText(/Skipped fields changed/)).toBeTruthy();
  });
});
