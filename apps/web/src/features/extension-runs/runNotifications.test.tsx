import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { toast } from '../../components/toast';
import { extensionRunStatuses } from './constants';
import { notifyRunFinished } from './runNotifications';
import type { ExtensionRun } from './schemas';

vi.mock('../../components/toast', () => ({
  toast: { error: vi.fn(), info: vi.fn(), success: vi.fn() },
}));

const failed = (failure_message: string | null) =>
  ({
    id: '00000000-0000-4000-8000-000000000001',
    status: extensionRunStatuses.failed,
    failure: 'extension_failed',
    failure_message,
  }) as ExtensionRun;

describe('notifyRunFinished', () => {
  beforeEach(() => vi.mocked(toast.error).mockReset());

  it("adds the extension's explanation to a failed run's toast", () => {
    notifyRunFinished(failed('NIP is invalid'), vi.fn());
    expect(vi.mocked(toast.error).mock.calls[0]?.[0]).toBe(
      'Extension run failed: NIP is invalid',
    );
  });

  it('keeps the plain notice when the extension gave none', () => {
    notifyRunFinished(failed(null), vi.fn());
    expect(vi.mocked(toast.error).mock.calls[0]?.[0]).toBe(
      'Extension run failed.',
    );
  });
});
