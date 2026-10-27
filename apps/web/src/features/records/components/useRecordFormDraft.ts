import { useMemo } from 'react';
import { draftEditors, type DraftEditor } from '../../drafts/constants';
import { formFieldsDraftSchema } from '../../drafts/schemas';
import { useEditorDraft } from '../../drafts/useEditorDraft';
import type { Attribute } from '../api';
import { attributeValueTypes } from '../valueTypes';

/** Identifies where unsaved field values are kept across refreshes. */
export type RecordFormDraft = {
  editor: DraftEditor;
  resource: readonly (string | number)[];
  /** Marker for the loaded source beyond the initial field values. */
  source: string;
};

// Joins the draft field codes into one string, so memoized values depend on
// the codes rather than on a new array each render.
const draftFieldSeparator = '\n';

const pickDraftFields = (
  fields: Record<string, string>,
  codes: readonly string[],
) =>
  Object.fromEntries(
    codes.flatMap((code) =>
      fields[code] === undefined ? [] : [[code, fields[code]]],
    ),
  );

/**
 * Keeps a record form's unsaved field values as a draft. Files upload
 * separately and are never kept; the draft is offered again only while its
 * source, including the form's initial values, is unchanged.
 */
export const useRecordFormDraft = ({
  editableAttributes,
  fields,
  initialFields,
  options,
  ready,
}: {
  editableAttributes: readonly Attribute[];
  fields: Record<string, string>;
  initialFields: Record<string, string>;
  /** Without options no draft is kept. */
  options: RecordFormDraft | undefined;
  ready: boolean;
}) => {
  const draftCodes = editableAttributes
    .filter((attribute) => attribute.value_type !== attributeValueTypes.file)
    .map((attribute) => attribute.code)
    .join(draftFieldSeparator);
  const draftFields = useMemo(
    () => pickDraftFields(fields, draftCodes.split(draftFieldSeparator)),
    [draftCodes, fields],
  );
  const initialDraftFields = JSON.stringify(
    pickDraftFields(initialFields, draftCodes.split(draftFieldSeparator)),
  );
  const fieldsChanged = JSON.stringify(draftFields) !== initialDraftFields;
  const draft = useEditorDraft({
    dirty: fieldsChanged,
    // Unused without draft options; `ready` keeps the draft off.
    editor: options?.editor ?? draftEditors.recordCreate,
    ready: Boolean(options && ready),
    resource: options?.resource ?? [],
    schema: formFieldsDraftSchema,
    source: JSON.stringify([options?.source, initialDraftFields]),
    value: draftFields,
  });
  /** The draft's values for the form's current fields, once restored. */
  const restore = () => {
    const restored = draft.restore();
    return restored
      ? pickDraftFields(restored, draftCodes.split(draftFieldSeparator))
      : undefined;
  };
  return { draft, fieldsChanged, restore };
};
