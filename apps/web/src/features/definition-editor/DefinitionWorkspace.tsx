import type { Monaco } from '@monaco-editor/react';
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { PanelRightCloseIcon, PanelRightOpenIcon } from 'lucide-react';
import type { editor } from 'monaco-editor';
import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  definitionEditorHeight,
  revealHighlightMs,
  type DefinitionKind,
} from './constants';
import { DefinitionEditor } from './DefinitionEditor';
import type { ReferenceAttribute } from './definitionReferences';
import { locateDefinitionError } from './definitionErrorLocation';
import { DefinitionFoldControls } from './DefinitionFoldControls';
import { DefinitionInsertMenu } from './DefinitionInsertMenu';
import { useDefinitionLayout } from './definitionLayout';
import { DefinitionOutlineButton } from './DefinitionOutlineButton';
import { DefinitionSplitHandle } from './DefinitionSplitHandle';
import type { DefinitionTarget } from './definitionNavigation';
import {
  DefinitionCursorContext,
  DefinitionRevealContext,
  type RevealDefinition,
} from './definitionReveal';
import { updateServerDiagnostic } from './definitionLanguage';
import { snippetEdit, type DefinitionSnippet } from './definitionSnippets';
import { checkStatuses, type DefinitionCheck } from './useDefinitionCheck';
import { captionIconSize, compactIconSize } from '../../components/iconSizes';

type Mounted = { editor: editor.IStandaloneCodeEditor; monaco: Monaco };

const lineOf = (text: string, offset: number) =>
  text.slice(0, offset).split('\n').length;

const firstLineEnd = (text: string) => {
  const end = text.indexOf('\n');
  return end < 0 ? text.length : end;
};

/** Monaco's class for briefly highlighting a range it navigated to. */
const rangeHighlightClass = 'rangeHighlight';

/**
 * Selects `focus` in the editor, scrolls it into view, and highlights the
 * block around it for a moment so the eye finds it.
 */
const revealDefinitionTarget = (
  { editor, monaco }: Mounted,
  { block, focus }: DefinitionTarget,
) => {
  const model = editor.getModel();
  if (!model) return;
  const toRange = ({ end, start }: { end: number; start: number }) =>
    monaco.Range.fromPositions(
      model.getPositionAt(start),
      model.getPositionAt(end),
    );
  const selection = toRange(focus);
  // Below the large breakpoint the preview sits under the editor.
  editor.getDomNode()?.scrollIntoView({ block: 'nearest' });
  editor.setSelection(selection);
  editor.revealRangeInCenterIfOutsideViewport(selection);
  editor.focus();
  const highlight = editor.createDecorationsCollection([
    {
      options: { className: rangeHighlightClass, isWholeLine: true },
      range: toRange(block),
    },
  ]);
  window.setTimeout(() => highlight.clear(), revealHighlightMs);
};

const CheckStatusChip = ({
  check,
  errorLine,
  onShowError,
}: {
  check: DefinitionCheck<unknown>;
  errorLine?: number;
  onShowError: () => void;
}) => {
  const { t } = useTranslation();
  switch (check.status) {
    case checkStatuses.idle:
      return null;
    case checkStatuses.checking:
      return (
        <Chip
          icon={<CircularProgress aria-hidden size={captionIconSize} />}
          label={t('definitionEditor.check.checking')}
          variant="outlined"
        />
      );
    case checkStatuses.valid:
      return (
        <Chip
          color="success"
          label={t('definitionEditor.check.valid')}
          variant="outlined"
        />
      );
    case checkStatuses.invalid:
      return (
        <Chip
          color="error"
          label={
            errorLine === undefined
              ? t('definitionEditor.check.invalid')
              : t('definitionEditor.check.invalidAtLine', { line: errorLine })
          }
          onClick={errorLine === undefined ? undefined : onShowError}
          variant="outlined"
        />
      );
  }
};

/**
 * The shared editing surface for blueprint, reusable attribute and rule
 * definitions: Insert and fold controls, the server's save check, and a live
 * preview of the last definition that passed it.
 */
export const DefinitionWorkspace = ({
  check,
  kind,
  onChange,
  onSave,
  preview,
  readOnly,
  savedAttributeCodes,
  scopeAttributes,
  toolbar,
  value,
}: {
  check: DefinitionCheck<unknown>;
  kind: DefinitionKind;
  onChange: (value: string) => void;
  /** Runs on Ctrl+S or Command+S. */
  onSave: () => void;
  /** Renders the last valid definition; omitted until one exists. */
  preview: ReactNode;
  readOnly: boolean;
  /** Attribute codes of the revision being revised, kept from renames. */
  savedAttributeCodes?: readonly string[];
  /** Attributes the definition refers to without declaring them. */
  scopeAttributes?: readonly ReferenceAttribute[];
  /** Extra toolbar actions placed before the Insert menu. */
  toolbar?: ReactNode;
  value: string;
}) => {
  const { t } = useTranslation();
  const previewHeadingId = useId();
  const [mounted, setMounted] = useState<Mounted>();
  const [cursor, setCursor] = useState<number>();
  const layout = useDefinitionLayout();
  // Monaco keeps commands registered on mount, so they read the latest
  // handlers through refs instead of capturing stale ones.
  const handlers = useRef({ onChange, onSave, readOnly });
  useLayoutEffect(() => {
    handlers.current = { onChange, onSave, readOnly };
  });
  const errorRange = useMemo(
    () =>
      check.serverMessage === undefined
        ? undefined
        : locateDefinitionError(value, check.serverMessage),
    [check.serverMessage, value],
  );
  const errorLine = errorRange && lineOf(value, errorRange.start);

  useEffect(() => {
    const model = mounted?.editor.getModel();
    if (!mounted || !model) return;
    updateServerDiagnostic(
      mounted.monaco,
      model,
      check.error === undefined
        ? undefined
        : {
            message: check.error,
            // An error that names no place marks the first line.
            range: errorRange ?? { end: firstLineEnd(value), start: 0 },
          },
    );
  }, [check.error, errorRange, mounted, value]);

  const showError = () => {
    if (!mounted || !errorRange) return;
    const model = mounted.editor.getModel();
    if (!model) return;
    const position = model.getPositionAt(errorRange.start);
    mounted.editor.setPosition(position);
    mounted.editor.revealLineInCenter(position.lineNumber);
    mounted.editor.focus();
  };

  const reveal = useMemo<RevealDefinition | undefined>(
    () => mounted && ((target) => revealDefinitionTarget(mounted, target)),
    [mounted],
  );

  const insert = (snippet: DefinitionSnippet) => {
    const model = mounted?.editor.getModel();
    if (!mounted || !model) return;
    const text = model.getValue();
    const edit = snippetEdit(text, snippet);
    const start = model.getPositionAt(edit.offset);
    const end = model.getPositionAt(edit.replaceEnd ?? edit.offset);
    mounted.editor.focus();
    mounted.editor.setSelection(
      new mounted.monaco.Selection(
        start.lineNumber,
        start.column,
        end.lineNumber,
        end.column,
      ),
    );
    // The snippet is already indented for where it goes; Monaco would
    // otherwise indent its lines again like the line it is inserted on.
    mounted.editor
      .getContribution<
        {
          insert: (
            template: string,
            options: { adjustWhitespace: boolean },
          ) => void;
        } & editor.IEditorContribution
      >('snippetController2')
      ?.insert(edit.insertText, { adjustWhitespace: false });
    mounted.editor.revealLineInCenter(
      mounted.editor.getPosition()?.lineNumber ?? start.lineNumber,
    );
  };

  return (
    <Box sx={{ mt: 3 }}>
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'center', flexWrap: 'wrap', mb: 1, rowGap: 1 }}
      >
        {toolbar}
        <DefinitionInsertMenu
          definition={value}
          disabled={readOnly || !mounted}
          kind={kind}
          onInsert={insert}
        />
        <DefinitionOutlineButton editor={mounted?.editor} />
        <DefinitionFoldControls editor={mounted?.editor} />
        <Box sx={{ flexGrow: 1 }} />
        <Button
          aria-pressed={layout.previewHidden}
          onClick={layout.togglePreview}
          startIcon={
            layout.previewHidden ? (
              <PanelRightOpenIcon aria-hidden size={compactIconSize} />
            ) : (
              <PanelRightCloseIcon aria-hidden size={compactIconSize} />
            )
          }
        >
          {layout.previewHidden
            ? t('definitionEditor.layout.showPreview')
            : t('definitionEditor.layout.hidePreview')}
        </Button>
        <CheckStatusChip
          check={check}
          errorLine={errorLine}
          onShowError={showError}
        />
      </Stack>
      <Box
        sx={{
          display: 'grid',
          // At the large breakpoint the split handle is the gap.
          gap: { lg: 0, xs: 2 },
          gridTemplateColumns: {
            lg: layout.previewHidden
              ? 'minmax(0, 1fr)'
              : `minmax(0, ${layout.editorShare}fr) auto minmax(0, ${1 - layout.editorShare}fr)`,
          },
        }}
      >
        <DefinitionEditor
          height={definitionEditorHeight}
          kind={kind}
          onChange={(next) => {
            if (!handlers.current.readOnly)
              handlers.current.onChange(next ?? '');
          }}
          onMount={(mountedEditor, monaco) => {
            mountedEditor.addCommand(
              monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
              () => handlers.current.onSave(),
            );
            mountedEditor.onDidChangeCursorPosition(({ position }) =>
              setCursor(mountedEditor.getModel()?.getOffsetAt(position)),
            );
            setMounted({ editor: mountedEditor, monaco });
          }}
          readOnly={readOnly}
          savedAttributeCodes={savedAttributeCodes}
          scopeAttributes={scopeAttributes}
          value={value}
        />
        {!layout.previewHidden && (
          <DefinitionSplitHandle
            editorShare={layout.editorShare}
            onCommit={layout.commitResize}
            onResize={layout.resize}
          />
        )}
        <Paper
          aria-labelledby={previewHeadingId}
          hidden={layout.previewHidden}
          component="section"
          sx={{
            maxHeight: { lg: definitionEditorHeight },
            overflow: 'auto',
            p: 2,
          }}
          variant="outlined"
        >
          <Typography component="h2" id={previewHeadingId} variant="subtitle1">
            {t('definitionEditor.preview.title')}
          </Typography>
          {check.error && (
            <Alert severity="error" sx={{ mt: 1 }}>
              {check.error}
              {errorLine !== undefined && (
                <Box sx={{ mt: 1 }}>
                  <Button
                    color="inherit"
                    onClick={showError}
                    size="small"
                    variant="outlined"
                  >
                    {t('definitionEditor.check.goToLine', { line: errorLine })}
                  </Button>
                </Box>
              )}
            </Alert>
          )}
          {check.result === undefined ? (
            <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
              {check.status === checkStatuses.checking
                ? t('definitionEditor.preview.checking')
                : t('definitionEditor.preview.empty')}
            </Typography>
          ) : (
            <>
              {check.stale && check.status === checkStatuses.invalid && (
                <Typography
                  color="text.secondary"
                  sx={{ mt: 1 }}
                  variant="body2"
                >
                  {t('definitionEditor.preview.stale')}
                </Typography>
              )}
              <DefinitionRevealContext value={reveal ?? null}>
                <DefinitionCursorContext value={cursor}>
                  {preview}
                </DefinitionCursorContext>
              </DefinitionRevealContext>
            </>
          )}
        </Paper>
      </Box>
    </Box>
  );
};
