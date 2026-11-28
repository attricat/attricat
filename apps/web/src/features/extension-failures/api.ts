import { request } from '../../api/request';
import { keysetQuery, type KeysetCursor } from '../../api/keysetCursor';
import { extensionPath } from '../extensions/apiPaths';
import type { HandlerFailure } from '../extension-exports/schemas';
import {
  deliveryFailurePageSchema,
  runFailurePageSchema,
  type RunFailure,
} from './schemas';

const failuresPath = (extensionId: string) =>
  `${extensionPath(extensionId)}/failures`;

/** Where the failed deliveries continue: a delivery is keyed by its handler too. */
export type DeliveryCursor = KeysetCursor & { before_handler: string };

export const runFailureCursor = (failure: RunFailure): KeysetCursor => ({
  before_time: failure.updated_at,
  before_id: failure.id,
});

export const deliveryFailureCursor = (
  failure: HandlerFailure,
): DeliveryCursor => ({
  before_time: failure.updated_at,
  before_id: failure.event_id,
  before_handler: failure.handler_id,
});

export const listRunFailures = (extensionId: string, cursor?: KeysetCursor) =>
  request(
    `${failuresPath(extensionId)}/runs${keysetQuery(cursor)}`,
    runFailurePageSchema,
  );

export const listDeliveryFailures = (
  extensionId: string,
  cursor?: DeliveryCursor,
) =>
  request(
    `${failuresPath(extensionId)}/deliveries${keysetQuery(cursor)}`,
    deliveryFailurePageSchema,
  );
