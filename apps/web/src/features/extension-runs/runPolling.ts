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

/** A 4xx (missing run or revoked access) will not change by polling. */
export const isRefusedExtensionRunError = (error: unknown) =>
  error instanceof ApiRequestError && isClientErrorStatus(error.status);

/** Refetch interval while `active`; a refused request stops polling too. */
export const extensionRunRefetchInterval = (active: boolean, error: unknown) =>
  active && !isRefusedExtensionRunError(error)
    ? extensionRunPollMilliseconds
    : false;
