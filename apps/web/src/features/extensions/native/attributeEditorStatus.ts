import { create } from 'zustand';
import type { Attribute } from '../../records/api';
import type { EditorReference } from '../../views/schemas';
import type { ExtensionContribution } from '../api';
import { contributionKinds, extensionOutlets } from '../constants';
import { useExtensionRuntime } from '../useExtensionRuntime';
import type { NativeUiErrorCode } from './constants';

/** Where a field's extension editor stands, as its details explain it. */
export const attributeEditorStates = {
  /** The editor draws the field. */
  active: 'active',
  /** It is starting; the default control stands in until it is ready. */
  starting: 'starting',
  /** It failed; the field uses its default control. */
  failed: 'failed',
  /** No enabled, granted extension declares it. */
  unavailable: 'unavailable',
  /** Its extension declares it, but not for the field's value type. */
  undeclared: 'undeclared',
} as const;
export type AttributeEditorState =
  (typeof attributeEditorStates)[keyof typeof attributeEditorStates];

/** States in which the field fell back to its default control. */
export const fallbackEditorStates: ReadonlySet<AttributeEditorState> = new Set([
  attributeEditorStates.failed,
  attributeEditorStates.unavailable,
  attributeEditorStates.undeclared,
]);

type Status = { state: AttributeEditorState; errorCode?: NativeUiErrorCode };

/** What each mounted extension editor reports, by record and field. */
export const useAttributeEditorStatusStore = create<{
  statuses: Record<string, Status>;
}>()(() => ({ statuses: {} }));

export const attributeEditorStatusKey = (recordId: string, code: string) =>
  `${recordId}:${code}`;

export const reportAttributeEditorStatus = (key: string, status?: Status) =>
  useAttributeEditorStatusStore.setState((state) => {
    const statuses = { ...state.statuses };
    if (status) statuses[key] = status;
    else delete statuses[key];
    return { statuses };
  });

export type AttributeEditorDiagnosis = {
  reference: EditorReference;
  contribution?: ExtensionContribution;
  state: AttributeEditorState;
  errorCode?: NativeUiErrorCode;
};

/**
 * Explains a field's extension editor: which one it is and whether it draws
 * the field, or why the field fell back to its default control.
 */
export const useAttributeEditorDiagnosis = (
  attribute: Attribute,
  reference: EditorReference | null | undefined,
  recordId: string | undefined,
): AttributeEditorDiagnosis | undefined => {
  const runtime = useExtensionRuntime();
  const status = useAttributeEditorStatusStore((state) =>
    recordId
      ? state.statuses[attributeEditorStatusKey(recordId, attribute.code)]
      : undefined,
  );
  if (!reference) return undefined;
  if (!runtime.data)
    return { reference, state: attributeEditorStates.starting };
  const contribution = runtime.data.find(
    (item) =>
      item.kind === contributionKinds.editor &&
      item.outlet === extensionOutlets.attributeEditor &&
      item.id === reference.id &&
      item.version === reference.version,
  );
  if (!contribution)
    return { reference, state: attributeEditorStates.unavailable };
  if (!contribution.editor?.value_types.includes(attribute.value_type))
    return {
      reference,
      contribution,
      state: attributeEditorStates.undeclared,
    };
  return {
    reference,
    contribution,
    state: status?.state ?? attributeEditorStates.starting,
    errorCode: status?.errorCode,
  };
};
