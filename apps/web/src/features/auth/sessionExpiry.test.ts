import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { ApiRequestError } from '../../api/request';
import { authQueryKeys } from './queryKeys';
import { recheckSessionWhenSignedOut } from './sessionExpiry';

describe('recheckSessionWhenSignedOut', () => {
  it('rechecks the session only when a request is unauthenticated', () => {
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const recheck = recheckSessionWhenSignedOut(client);
    recheck(new ApiRequestError(403, 'no', 'forbidden'));
    recheck(new ApiRequestError(401, 'bad', 'invalid_credentials'));
    expect(invalidate).not.toHaveBeenCalled();
    recheck(new ApiRequestError(401, 'sign in', 'unauthenticated'));
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: authQueryKeys.session(),
    });
  });
});
