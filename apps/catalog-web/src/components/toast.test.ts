import { describe, expect, it } from 'vitest';
import { subscribeToToasts, toast } from './toast';

describe('toast', () => {
  it('delivers notifications emitted before the provider subscribes', () => {
    toast.success('Saved');
    const received: string[] = [];
    const unsubscribe = subscribeToToasts((notification) => {
      received.push(notification.message);
    });

    toast.error('Failed');
    unsubscribe();

    expect(received).toEqual(['Saved', 'Failed']);
  });
});
