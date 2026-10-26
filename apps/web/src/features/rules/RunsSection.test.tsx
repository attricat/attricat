// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { RunsSection } from './RunsSection';

describe('RunsSection', () => {
  it('offers to retry when the runs fail to load', async () => {
    const onRetry = vi.fn();
    render(
      <RunsSection
        error={new Error('offline')}
        onRetry={onRetry}
        recordLabels={new Map()}
        revisions={new Map()}
        runs={undefined}
      />,
    );
    expect(screen.getByText('Unable to load this information')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onRetry).toHaveBeenCalledOnce();
  });
});
