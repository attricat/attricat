import type { ReactNode } from 'react';
import type { Attribute } from '../../records/api';
import { useExtensionRuntime } from '../useExtensionRuntime';
import {
  resolveAttributeEditor,
  type EditorReference,
} from './attributeEditors';
import { ExtensionAttributeEditor } from './ExtensionAttributeEditor';
import { nativeHostKey } from './nativeRuntime';

type Props = {
  attribute: Attribute;
  reference: EditorReference;
  contextId: string | null;
  value: string;
  error?: string;
  required: boolean;
  readOnly: boolean;
  onChange: (text: string) => void;
  /** The field's default control, shown whenever the editor cannot draw it. */
  fallback: ReactNode;
};

/**
 * A detail field's extension editor while its record is being entered. The
 * record has no ID yet, and nothing saves until the form is submitted, so a
 * committed proposal is a change like any other. Reads the extension runtime
 * only for a field that names an editor.
 */
export const NewRecordAttributeEditor = ({
  attribute,
  reference,
  contextId,
  value,
  error,
  required,
  readOnly,
  onChange,
  fallback,
}: Props) => {
  const runtime = useExtensionRuntime();
  const contribution = resolveAttributeEditor(
    runtime.data,
    reference,
    attribute,
  );
  if (!contribution) return fallback;
  return (
    <ExtensionAttributeEditor
      attribute={attribute}
      contextId={contextId}
      contribution={contribution}
      error={error}
      fallback={fallback}
      key={nativeHostKey(contribution)}
      onChange={onChange}
      onCommit={onChange}
      readOnly={readOnly}
      recordId={null}
      reference={reference}
      required={required}
      value={value}
    />
  );
};
