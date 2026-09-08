// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { DataHealthControls } from './DataHealthControls';

describe('DataHealthControls', () => {
  it('provides a labelled custom threshold field with bounded updates', () => {
    const onStaleAfterDaysChange = vi.fn();

    render(
      <DataHealthControls
        isRefreshing={false}
        onRefresh={vi.fn()}
        onStaleAfterDaysChange={onStaleAfterDaysChange}
        refreshError={null}
        staleAfterDays={7}
      />,
    );

    const threshold = screen.getByRole('spinbutton', { name: 'Days' });
    expect(threshold).toHaveProperty('min', '1');
    expect(threshold).toHaveProperty('max', '3650');

    fireEvent.change(threshold, { target: { value: '0' } });
    fireEvent.blur(threshold);
    expect(onStaleAfterDaysChange).not.toHaveBeenCalled();

    fireEvent.change(threshold, { target: { value: '14' } });
    fireEvent.blur(threshold);
    expect(onStaleAfterDaysChange).toHaveBeenCalledWith(14);
  });

  it('disables refresh while it is already in progress', () => {
    render(
      <DataHealthControls
        isRefreshing
        onRefresh={vi.fn()}
        onStaleAfterDaysChange={vi.fn()}
        refreshError={new Error('Refresh failed')}
        staleAfterDays={90}
      />,
    );

    const refreshButton = screen.getByRole('button', { name: 'Refreshing...' });
    expect(refreshButton).toHaveProperty('disabled', true);
    expect(screen.getByRole('alert').textContent).toContain('Refresh failed');
  });
});
