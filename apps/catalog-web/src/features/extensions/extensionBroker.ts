import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import { requestText } from '../../api/request';
import { toast } from '../../components/toast';
import {
  maximumNotificationMessageLength,
  notificationSeverities,
} from '../web-components/constants';
import {
  extensionCommand,
  extensionCommandRequestSchema,
  extensionStorage,
  extensionStorageRequestSchema,
  type ExtensionContribution,
} from './api';
import {
  blueprintRevisionPathPattern,
  catalogReadPathPattern,
  controlCharacterRanges,
  extensionBrokerMethods,
  extensionCapabilities,
  extensionMessageTypes,
  extensionProtocolErrors,
  extensionStorageMethodPrefix,
  extensionStorageOperations,
  maximumExtensionRequestBytes,
  maximumExtensionResponseBytes,
  maximumExtensionStorageKeyBytes,
} from './constants';
import { refreshCurrentEntity } from './refreshEntity';
import { jsonByteLength, utf8ByteLength } from './utf8';

type FrameContext = Record<string, unknown>;

const notificationSchema = z
  .object({
    message: z.string().trim().min(1).max(maximumNotificationMessageLength),
    severity: z.enum(notificationSeverities).optional(),
  })
  .strict();
const navigationSchema = z.object({ entity_id: z.uuid() }).strict();
const catalogReadSchema = z
  .object({ path: z.string().regex(catalogReadPathPattern) })
  .strict();

const isControlCharacter = (character: string) => {
  const code = character.charCodeAt(0);
  return (
    code <= controlCharacterRanges.c0End ||
    (code >= controlCharacterRanges.c1Start &&
      code <= controlCharacterRanges.c1End)
  );
};

const validStorageKey = (key: string) =>
  key.length > 0 &&
  key === key.trim() &&
  utf8ByteLength(key) <= maximumExtensionStorageKeyBytes &&
  ![...key].some(isControlCharacter);

export const validateStorageRequest = (payload: unknown) => {
  const request = extensionStorageRequestSchema.parse(payload);
  const keys = [
    'key' in request ? request.key : undefined,
    'prefix' in request ? request.prefix : undefined,
    'cursor' in request ? request.cursor : undefined,
  ];
  if (keys.some((key) => key !== undefined && !validStorageKey(key)))
    throw new Error(extensionProtocolErrors.invalidStorageKey);
  if (
    request.operation === 'set' &&
    jsonByteLength(request.value) > maximumExtensionRequestBytes
  )
    throw new Error(extensionProtocolErrors.storageValueTooLarge);
  if (jsonByteLength(request) > maximumExtensionRequestBytes)
    throw new Error(extensionProtocolErrors.storageRequestTooLarge);
  return request;
};

export type BrokerRequest = { method: unknown; payload: unknown };

export type BrokerDependencies = {
  contribution: ExtensionContribution;
  /** The context the frame was started with; it scopes blueprint reads. */
  initialContext: FrameContext;
  /** Reads the latest host context when the request is handled. */
  currentContext: () => FrameContext;
  navigateToEntity: (entityId: string) => Promise<void>;
  queryClient: QueryClient;
};

const payloadObject = (payload: unknown) => payload as Record<string, unknown>;

const handleStorage = async (
  operation: string,
  payload: unknown,
  { contribution }: BrokerDependencies,
) => {
  if (!(extensionStorageOperations as readonly string[]).includes(operation))
    throw new Error(extensionProtocolErrors.storageRequestDenied);
  const request = validateStorageRequest({
    ...payloadObject(payload),
    operation,
  });
  return extensionStorage(
    contribution.extension_id,
    contribution.id,
    contribution.release_id,
    request,
  );
};

const handleCommand = async (
  payload: unknown,
  { contribution }: BrokerDependencies,
) => {
  const command = extensionCommandRequestSchema.parse({
    ...payloadObject(payload),
    release_id: contribution.release_id,
  });
  if (jsonByteLength(command.payload) > maximumExtensionRequestBytes)
    throw new Error(extensionProtocolErrors.commandPayloadTooLarge);
  return extensionCommand(contribution.extension_id, contribution.id, command);
};

const handleCatalogRead = async (
  payload: unknown,
  { initialContext }: BrokerDependencies,
) => {
  const { path } = catalogReadSchema.parse(payload);
  const revision = path.match(blueprintRevisionPathPattern);
  if (
    revision &&
    (initialContext.blueprint_id !== revision[1] ||
      String(initialContext.blueprint_version) !== revision[2])
  )
    throw new Error(extensionProtocolErrors.blueprintRevisionOutsideContext);
  const text = await requestText(path);
  if (utf8ByteLength(text) > maximumExtensionResponseBytes)
    throw new Error(extensionProtocolErrors.catalogResponseTooLarge);
  return JSON.parse(text) as unknown;
};

/**
 * Executes one frame request after checking the contribution's capabilities.
 * Resolves with the response data, or rejects when the request is denied or
 * fails; callers must never reveal the rejection reason to the frame.
 */
export const handleBrokerRequest = async (
  { method, payload }: BrokerRequest,
  dependencies: BrokerDependencies,
): Promise<unknown> => {
  const { contribution } = dependencies;
  const can = (capability: string) =>
    contribution.capabilities.includes(capability);
  if (typeof method !== 'string')
    throw new Error(extensionProtocolErrors.requestDenied);
  if (
    method === extensionBrokerMethods.navigate &&
    can(extensionCapabilities.navigation)
  ) {
    const detail = navigationSchema.parse(payload);
    await dependencies.navigateToEntity(detail.entity_id);
    return null;
  }
  if (method === extensionBrokerMethods.refresh) {
    await refreshCurrentEntity(
      dependencies.queryClient,
      contribution,
      dependencies.currentContext(),
      payload,
    );
    return null;
  }
  if (
    method === extensionBrokerMethods.notify &&
    can(extensionCapabilities.notification)
  ) {
    toast.show(notificationSchema.parse(payload));
    return null;
  }
  if (
    method.startsWith(extensionStorageMethodPrefix) &&
    can(extensionCapabilities.storage)
  )
    return handleStorage(
      method.slice(extensionStorageMethodPrefix.length),
      payload,
      dependencies,
    );
  if (
    method === extensionBrokerMethods.command &&
    contribution.kind !== 'panel' &&
    can(extensionCapabilities.commands)
  )
    return handleCommand(payload, dependencies);
  if (
    method === extensionBrokerMethods.catalogRead &&
    can(extensionCapabilities.catalogRead)
  )
    return handleCatalogRead(payload, dependencies);
  throw new Error(extensionProtocolErrors.requestDenied);
};

/** Posts a broker response, denying any successful value that is too large. */
export const postBrokerResponse = (
  port: MessagePort,
  id: string,
  result: { ok: true; data: unknown } | { ok: false },
) => {
  const ok =
    result.ok && jsonByteLength(result.data) <= maximumExtensionResponseBytes;
  port.postMessage({
    type: extensionMessageTypes.response,
    id,
    ok,
    ...(ok && result.ok
      ? { data: result.data }
      : { error: extensionProtocolErrors.requestDenied }),
  });
};
