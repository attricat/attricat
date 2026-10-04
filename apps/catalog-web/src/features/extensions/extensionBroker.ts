import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import { requestText } from '../../api/request';
import { toast } from '../../components/toast';
import {
  maximumNotificationMessageLength,
  notificationSeverities,
} from '../web-components/constants';
import {
  cancelExtensionRun,
  extensionRunArtifactUrl,
  getExtensionRun,
  listExtensionRuns,
  startExtensionRun,
} from '../extension-runs/api';
import {
  isSelectionContribution,
  parseSelectionContext,
  type SelectionContext,
} from './actionSelection';
import {
  extensionCommand,
  extensionCommandRequestSchema,
  extensionStorage,
  extensionStorageRequestSchema,
  type ExtensionContribution,
} from './api';
import {
  actionDialogOutlet,
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
  maximumExtensionCommandIdLength,
  maximumExtensionStorageKeyBytes,
  maximumOperationIdempotencyKeyLength,
  operationIdempotencyKeyPattern,
  operationMethodPrefix,
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
const runReferenceSchema = z.object({ run_id: z.uuid() }).strict();
const artifactReferenceSchema = z
  .object({ run_id: z.uuid(), artifact_id: z.uuid() })
  .strict();
const startOperationSchema = z
  .object({
    operation_id: z.string().min(1).max(maximumExtensionCommandIdLength),
    input: z.record(z.string(), z.unknown()),
    idempotency_key: z
      .string()
      .min(1)
      .max(maximumOperationIdempotencyKeyLength)
      .regex(operationIdempotencyKeyPattern),
  })
  .strict();
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
  /** Opens the same extension's host-managed action dialog. */
  openActionDialog: (request: ActionDialogRequest) => void;
  closeActionDialog: () => void;
  /** Lets the host track a run after the source frame unmounts. */
  onRunStarted: (runId: string) => void;
  downloadArtifact: (url: string) => void;
};

export type ActionDialogRequest = {
  extensionId: string;
  context: SelectionContext;
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

/** The selection a frame may act on: its own selection context, if any. */
const frameSelection = ({
  contribution,
  currentContext,
}: BrokerDependencies) => {
  if (
    !isSelectionContribution(contribution) &&
    contribution.outlet !== actionDialogOutlet
  )
    throw new Error(extensionProtocolErrors.selectionUnavailable);
  const selection = parseSelectionContext(currentContext());
  if (!selection) throw new Error(extensionProtocolErrors.selectionUnavailable);
  return selection;
};

/**
 * Reads a run only when the signed-in user started it for the calling
 * extension. The `own` scope makes the server refuse other users' runs even
 * for an operator; the client checks again.
 */
const ownRun = async (runId: string, { contribution }: BrokerDependencies) => {
  const run = await getExtensionRun(runId, 'own');
  if (run.extension_id !== contribution.extension_id || !run.initiated_by_me)
    throw new Error(extensionProtocolErrors.requestDenied);
  return run;
};

const handleOperation = async (
  method: string,
  payload: unknown,
  dependencies: BrokerDependencies,
) => {
  const { contribution } = dependencies;
  const can = (capability: string) =>
    contribution.capabilities.includes(capability);
  if (
    method === extensionBrokerMethods.operationsStart &&
    can(extensionCapabilities.operationsStart)
  ) {
    const selection = frameSelection(dependencies);
    const request = startOperationSchema.parse(payload);
    if (jsonByteLength(request.input) > maximumExtensionRequestBytes)
      throw new Error(extensionProtocolErrors.operationInputTooLarge);
    const { run_id } = await startExtensionRun(
      contribution.extension_id,
      contribution.id,
      {
        release_id: contribution.release_id,
        operation_id: request.operation_id,
        input: request.input,
        idempotency_key: request.idempotency_key,
        selection: {
          blueprint_id: selection.blueprint_id,
          blueprint_version: selection.blueprint_version,
          context_id: selection.context_id,
          entity_ids: selection.entity_ids,
        },
      },
    );
    dependencies.onRunStarted(run_id);
    return { run_id };
  }
  if (
    method === extensionBrokerMethods.operationsList &&
    can(extensionCapabilities.operationsRead)
  )
    return listExtensionRuns(contribution.extension_id);
  if (
    method === extensionBrokerMethods.operationsGet &&
    can(extensionCapabilities.operationsRead)
  )
    return ownRun(runReferenceSchema.parse(payload).run_id, dependencies);
  if (
    method === extensionBrokerMethods.operationsCancel &&
    can(extensionCapabilities.operationsCancel)
  ) {
    const { run_id } = runReferenceSchema.parse(payload);
    await ownRun(run_id, dependencies);
    await cancelExtensionRun(run_id, 'own');
    return null;
  }
  if (
    method === extensionBrokerMethods.operationsDownload &&
    can(extensionCapabilities.operationsRead)
  ) {
    const { run_id, artifact_id } = artifactReferenceSchema.parse(payload);
    const run = await ownRun(run_id, dependencies);
    if (!run.artifacts.some((artifact) => artifact.id === artifact_id))
      throw new Error(extensionProtocolErrors.requestDenied);
    dependencies.downloadArtifact(
      extensionRunArtifactUrl(run_id, artifact_id, 'own'),
    );
    return null;
  }
  throw new Error(extensionProtocolErrors.requestDenied);
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
  if (
    method === extensionBrokerMethods.dialogOpen &&
    can(extensionCapabilities.actionDialog) &&
    isSelectionContribution(contribution)
  ) {
    // The dialog captures the selection now; later Explorer changes never
    // alter what the user is configuring.
    dependencies.openActionDialog({
      extensionId: contribution.extension_id,
      context: frameSelection(dependencies),
    });
    return null;
  }
  if (
    method === extensionBrokerMethods.dialogClose &&
    contribution.outlet === actionDialogOutlet
  ) {
    dependencies.closeActionDialog();
    return null;
  }
  if (method.startsWith(operationMethodPrefix))
    return handleOperation(method, payload, dependencies);
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
