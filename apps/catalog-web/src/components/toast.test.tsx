// @vitest-environment jsdom
import { act, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { ToastProvider } from './ToastProvider';
import { toast } from './toast';
import { useToastStore } from './toastStore';
import { useToast } from './useToast';
import '../i18n';

beforeEach(() => useToastStore.setState({ toasts: [] }));

describe('toast', () => {
  it('renders notifications emitted before and after the UI mounts', () => {
    toast.success('Saved');
    render(
      <ToastProvider>
        <div>App</div>
      </ToastProvider>,
    );
    expect(screen.getByText('Saved')).toBeTruthy();

    act(() => toast.error('Failed'));
    expect(screen.getByText('Failed')).toBeTruthy();
  });

  it('shares the queue with hook callers and preserves grouping and dismissal', () => {
    const Actions = () => {
      const { show, dismiss } = useToast();
      return (
        <>
          <button onClick={() => show({ message: 'Saved' })}>Show</button>
          <button
            onClick={() => dismiss(useToastStore.getState().toasts[0].id)}
          >
            Dismiss
          </button>
        </>
      );
    };
    render(
      <ToastProvider>
        <Actions />
      </ToastProvider>,
    );

    act(() => screen.getByText('Show').click());
    act(() => toast.info('Saved'));
    expect(useToastStore.getState().toasts).toHaveLength(1);
    expect(useToastStore.getState().toasts[0].count).toBe(2);

    act(() => screen.getByText('Dismiss').click());
    expect(screen.queryByText('Saved')).toBeNull();
  });
});
