import { ApiRequestError } from '../../api/request';
import {
  activeExtensionRunStatuses,
  extensionRunPollMilliseconds,
  type ExtensionRunStatus,
} from './constants';
import { isClientErrorStatus } from '../../api/httpStatus';

/** Whether a run can still change: queued, running or cancelling. */
export const isActiveExtensionRun = (run: { status: ExtensionRunStatus }) =>
  activeExtensionRunStatuses.includes(run.status);

/**
 * Refetch interval while `active`. A 4xx (missing run or revoked access) will
 * not change by polling, so it stops polling too.
 */
export const extensionRunRefetchInterval = (active: boolean, error: unknown) =>
  active &&
  !(error instanceof ApiRequestError && isClientErrorStatus(error.status))
    ? extensionRunPollMilliseconds
    : false;
