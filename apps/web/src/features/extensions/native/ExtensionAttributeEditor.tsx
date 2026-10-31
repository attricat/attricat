import { Box, FormHelperText, FormLabel } from '@mui/material';
import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import type { Attribute } from '../../records/api';
import { attributeLabel } from '../../records/recordDisplay';
import type { ExtensionContribution } from '../api';
import {
  extensionFramePlaceholders,
  supportedOutletContextVersion,
} from '../constants';
import type { AttributeEditorContext } from '../outletContextSchemas';
import { useExtensionRuntime } from '../useExtensionRuntime';
import {
  editorValue,
  proposedText,
  type EditorReference,
} from './attributeEditors';
import type { NativeFieldLabel } from './nativeContexts';
import {
  attributeEditorStates,
  attributeEditorStatusKey,
  reportAttributeEditorStatus,
} from './attributeEditorStatus';
import type { NativeUiErrorCode } from './constants';
import { nativeHostFor } from './nativeRuntime';
import { NativeUi } from './NativeUi';

/** Keeps the label for assistive technology without drawing it. */
const visuallyHidden = {
  clip: 'rect(0 0 0 0)',
  height: 1,
  overflow: 'hidden',
  position: 'absolute',
  whiteSpace: 'nowrap',
  width: 1,
} as const;

type Props = {
  contribution: ExtensionContribution;
  reference: EditorReference;
  attribute: Attribute;
  recordId: string;
  contextId: string | null;
  /** The field's inline-form text. */
  value: string;
  error?: string;
  required: boolean;
  /** Keeps a proposal as a local edit, as typing does. */
  onChange: (text: string) => void;
  /** Validates and saves a proposal through the inline-edit path. */
  onCommit: (text: string) => void;
  /** The field's default control, shown until the editor is ready. */
  fallback: ReactNode;
};

const editorStates = {
  starting: 'starting',
  ready: 'ready',
  failed: 'failed',
} as const;
type EditorState = (typeof editorStates)[keyof typeof editorStates];

/**
 * An extension's native editor in a host field: the host draws the label,
 * help and errors and saves what the editor proposes through the inline-edit
 * path, so blur and Enter save and Escape restores as in any field. Until the
 * editor is ready, and whenever it fails, the field keeps its default
 * control; a blueprint never depends on an extension being healthy.
 */
export const ExtensionAttributeEditor = ({
  contribution,
  reference,
  attribute,
  recordId,
  contextId,
  value,
  error,
  required,
  onChange,
  onCommit,
  fallback,
}: Props) => {
  const runtime = useExtensionRuntime();
  const labelId = useId();
  const box = useRef<HTMLDivElement>(null);
  const fallbackBox = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<EditorState>(editorStates.starting);
  const [errorCode, setErrorCode] = useState<NativeUiErrorCode>();
  // The field's details explain whether the editor draws it.
  useEffect(() => {
    const key = attributeEditorStatusKey(recordId, attribute.code);
    reportAttributeEditorStatus(key, {
      state:
        state === editorStates.ready
          ? attributeEditorStates.active
          : state === editorStates.failed
            ? attributeEditorStates.failed
            : attributeEditorStates.starting,
      errorCode,
    });
    return () => reportAttributeEditorStatus(key);
  }, [attribute.code, errorCode, recordId, state]);
  // Readiness never takes the default control from under the user's typing.
  const [waitingForBlur, setWaitingForBlur] = useState(false);
  const commitNext = useRef(false);
  // The first input that hides its own label draws the field's label on its
  // outline, like every other field; the host's label then only names the
  // group for assistive technology.
  const labelOwner = useRef<string>(undefined);
  const [labelClaimed, setLabelClaimed] = useState(false);
  const fieldLabel = useMemo<NativeFieldLabel>(
    () => ({
      text: attributeLabel(attribute),
      required,
      error: Boolean(error),
      claim: (id, grant) => {
        if (labelOwner.current !== undefined && labelOwner.current !== id)
          return;
        labelOwner.current = id;
        grant(true);
        setLabelClaimed(true);
      },
      release: (id) => {
        if (labelOwner.current !== id) return;
        labelOwner.current = undefined;
        setLabelClaimed(false);
      },
    }),
    [attribute, required, error],
  );
  const constraints = attribute.value_schema ?? null;
  const context = useMemo<AttributeEditorContext>(
    () => ({
      context_version: supportedOutletContextVersion,
      record_id: recordId,
      context_id: contextId,
      value: editorValue(attribute, value),
      attribute: {
        code: attribute.code,
        name: attributeLabel(attribute),
        value_type: attribute.value_type,
        constraints: constraints as Record<string, unknown> | null,
        options: Array.isArray((constraints as { enum?: unknown } | null)?.enum)
          ? (constraints as { enum: (string | number)[] }).enum
          : null,
      },
      read_only: false,
      required,
      errors: error ? [error] : [],
      props: reference.props,
    }),
    [
      attribute,
      constraints,
      contextId,
      error,
      recordId,
      reference.props,
      required,
      value,
    ],
  );

  const propose = (proposed: unknown) => {
    const text = proposedText(attribute, proposed);
    if (text === undefined) return;
    const focused = box.current?.contains(document.activeElement) ?? false;
    if (!focused || commitNext.current) {
      commitNext.current = false;
      onCommit(text);
    } else onChange(text);
  };

  const ready = state === editorStates.ready && !waitingForBlur;
  return (
    <>
      {!ready && (
        <Box
          onBlur={(event) => {
            if (
              waitingForBlur &&
              !event.currentTarget.contains(event.relatedTarget as Node | null)
            )
              setWaitingForBlur(false);
          }}
          ref={fallbackBox}
        >
          {fallback}
        </Box>
      )}
      {state !== editorStates.failed && (
        <Box
          aria-labelledby={labelId}
          hidden={!ready}
          onKeyDown={(event) => {
            if (
              event.key !== 'Enter' ||
              !(event.target instanceof HTMLInputElement)
            )
              return;
            // The editor's answer to the input's last change saves too.
            commitNext.current = true;
            void nativeHostFor(contribution, runtime.data ?? [contribution])
              .settle()
              .then(() => {
                commitNext.current = false;
              });
          }}
          ref={box}
          role="group"
        >
          <FormLabel
            error={Boolean(error)}
            id={labelId}
            required={required}
            sx={labelClaimed ? visuallyHidden : undefined}
          >
            {attributeLabel(attribute)}
          </FormLabel>
          <NativeUi
            context={context}
            contribution={contribution}
            onFailure={(code) => {
              setErrorCode(code);
              setState(editorStates.failed);
            }}
            fieldLabel={fieldLabel}
            onPropose={propose}
            onReady={() => {
              setState(editorStates.ready);
              if (fallbackBox.current?.contains(document.activeElement))
                setWaitingForBlur(true);
            }}
            placeholder={extensionFramePlaceholders.none}
          />
          {error && <FormHelperText error>{error}</FormHelperText>}
        </Box>
      )}
    </>
  );
};
