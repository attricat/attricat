import { FindingSeverityChip } from '../../rules/FindingSeverityChip';
import {
  Box,
  IconButton,
  Popover,
  Stack,
  Tooltip,
  Typography,
} from '@mui/material';
import { CircleAlertIcon, CodeXmlIcon, CopyIcon, InfoIcon } from 'lucide-react';
import { useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../../app/theme';
import { compactIconSize, smallIconSize } from '../../../components/iconSizes';
import { useCopyToClipboard } from '../../../components/useCopyToClipboard';
import { findingMessage } from '../../rules/findingMessage';
import { findingSeverityColor } from '../../rules/findingSeverity';
import type { Attribute } from '../api';
import {
  attributeCardinalities,
  attributeContextEditability,
  attributeContextFallbacks,
} from '../constants';
import {
  fieldFindingsStatuses,
  useFieldDiagnostics,
  useFieldFindings,
  worstSeverity,
} from '../fieldDiagnostics';
import { errorMessage } from '../../../api/errors';
import { defaultContextCode } from '../../contexts/constants';
import { attributeLabel } from '../recordDisplay';
import { attributeValueTypes } from '../valueTypes';
import { useRevealField } from '../../views/fieldReveal';
import { FieldConnectionList } from './FieldConnectionList';
import { ValueTypeLabel } from './ValueTypeLabel';
import type { ResolvedValue } from '../../views/components/ValueField';
import type { EditorReference } from '../../views/schemas';
import { RouterLink } from '../../../components/RouterLink';
import {
  attributeEditorStates,
  fallbackEditorStates,
  useAttributeEditorDiagnosis,
  type AttributeEditorDiagnosis,
} from '../../extensions/native/attributeEditorStatus';

const diagnosticsPopoverMaxWidth = 360;

const isEmptyValue = (value: unknown) =>
  value === undefined ||
  value === null ||
  value === '' ||
  (Array.isArray(value) && value.length === 0);

const DiagnosticSection = ({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) => (
  <Box component="section">
    <Typography
      color="text.secondary"
      component="h3"
      sx={{ fontWeight: 700, mb: 0.5 }}
      variant="caption"
    >
      {title}
    </Typography>
    {children}
  </Box>
);

/** Which extension editor a field names, and whether it draws the field. */
const EditorDiagnosis = ({
  diagnosis,
}: {
  diagnosis: AttributeEditorDiagnosis;
}) => {
  const { t } = useTranslation();
  const { contribution, reference } = diagnosis;
  const editorName = contribution
    ? t('records.fieldDiagnostics.editorName', {
        editor: contribution.title ?? contribution.id,
        extension: contribution.extension_name,
        id: reference.id,
        version: reference.version,
      })
    : t('records.fieldDiagnostics.editorReference', {
        id: reference.id,
        version: reference.version,
      });
  const explanation = {
    [attributeEditorStates.active]: t('records.fieldDiagnostics.editorActive'),
    [attributeEditorStates.starting]: t(
      'records.fieldDiagnostics.editorStarting',
    ),
    [attributeEditorStates.failed]: t('records.fieldDiagnostics.editorFailed', {
      reason: diagnosis.errorCode
        ? t(`extensions.native.errors.${diagnosis.errorCode}`, {
            extension: contribution?.extension_name ?? reference.id,
          })
        : '',
    }),
    [attributeEditorStates.unavailable]: t(
      'records.fieldDiagnostics.editorUnavailable',
    ),
    [attributeEditorStates.undeclared]: t(
      'records.fieldDiagnostics.editorUndeclared',
    ),
  }[diagnosis.state];
  return (
    <Stack spacing={0.5}>
      <Typography variant="body2">{editorName}</Typography>
      <Typography
        color={
          fallbackEditorStates.has(diagnosis.state)
            ? 'warning.main'
            : 'text.secondary'
        }
        variant="body2"
      >
        {explanation}
      </Typography>
      {contribution && (
        <RouterLink
          params={{ extensionId: contribution.extension_id }}
          to="/manage/extensions/$extensionId"
          variant="body2"
        >
          {t('records.fieldDiagnostics.openExtension')}
        </RouterLink>
      )}
    </Stack>
  );
};

/**
 * A field's code, value type, value source and data quality findings, from
 * a small button beside the field. Renders nothing outside the record page.
 */
type FieldDiagnosticsProps = {
  attribute: Attribute;
  resolved?: ResolvedValue;
  /** An extension's attribute editor the view names for this field. */
  editor?: EditorReference | null;
  recordId?: string;
};

/** Reads the extension runtime only for a field that names an editor. */
const FieldWithEditorDiagnostics = (
  props: FieldDiagnosticsProps & { editor: EditorReference },
) => (
  <FieldDiagnostics
    {...props}
    editorDiagnosis={useAttributeEditorDiagnosis(
      props.attribute,
      props.editor,
      props.recordId,
    )}
  />
);

export const FieldDiagnosticsButton = (props: FieldDiagnosticsProps) =>
  props.editor ? (
    <FieldWithEditorDiagnostics {...props} editor={props.editor} />
  ) : (
    <FieldDiagnostics {...props} />
  );

const FieldDiagnostics = ({
  attribute,
  resolved,
  editorDiagnosis,
}: FieldDiagnosticsProps & {
  editorDiagnosis?: AttributeEditorDiagnosis;
}) => {
  const { t } = useTranslation();
  const diagnostics = useFieldDiagnostics();
  const findings = useFieldFindings(attribute.code);
  // A field that fell back from its extension editor reads as a warning, so
  // the problem shows without opening the details.
  const editorProblem =
    editorDiagnosis !== undefined &&
    fallbackEditorStates.has(editorDiagnosis.state);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  // Set when the popover closes to lead to another field, which keeps focus.
  const [revealing, setRevealing] = useState(false);
  // Runs once the popover has closed, whose focus trap would take focus back.
  const afterClose = useRef<() => void>(undefined);
  const copyText = useCopyToClipboard();
  const revealField = useRevealField();
  if (!diagnostics) return null;
  const { contextCode } = diagnostics;
  const showInDefinition = diagnostics.showInDefinition?.(attribute.code);
  const label = attributeLabel(attribute);
  const connections =
    diagnostics.connections?.filter((connection) =>
      connection.attributeCodes.includes(attribute.code),
    ) ?? [];
  const severity = findings && worstSeverity(findings);
  const Icon = severity || editorProblem ? CircleAlertIcon : InfoIcon;
  const sourceContext = resolved?.source_context;
  const fallsBack =
    attribute.context_fallback !== attributeContextFallbacks.none;
  const editableEverywhere =
    attribute.context_editable !== attributeContextEditability.default;
  return (
    <>
      <Tooltip title={t('records.fieldDiagnostics.tooltip')}>
        <IconButton
          aria-label={
            findings?.length
              ? t('records.fieldDiagnostics.openWithFindings', {
                  count: findings.length,
                  field: label,
                })
              : t('records.fieldDiagnostics.open', { field: label })
          }
          onClick={(event) => {
            setRevealing(false);
            setAnchor(event.currentTarget);
          }}
          size="small"
          sx={{
            color: severity
              ? `${findingSeverityColor(severity)}.main`
              : editorProblem
                ? 'warning.main'
                : 'text.secondary',
          }}
        >
          <Icon size={compactIconSize} />
        </IconButton>
      </Tooltip>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        disableRestoreFocus={revealing}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
        slotProps={{
          paper: { sx: { maxWidth: diagnosticsPopoverMaxWidth, p: 2 } },
          transition: {
            onExited: () => {
              afterClose.current?.();
              afterClose.current = undefined;
            },
          },
        }}
      >
        <Stack spacing={2}>
          <Typography component="h2" sx={{ fontWeight: 700 }} variant="body2">
            {label}
          </Typography>
          <DiagnosticSection title={t('records.fieldDiagnostics.code')}>
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <Typography
                component="code"
                sx={{ fontFamily: monoFontFamily, overflowWrap: 'anywhere' }}
                variant="body2"
              >
                {attribute.code}
              </Typography>
              <Tooltip title={t('records.fieldDiagnostics.copyCode')}>
                <IconButton
                  aria-label={t('records.fieldDiagnostics.copyCode')}
                  onClick={() => void copyText(attribute.code)}
                  size="small"
                >
                  <CopyIcon size={smallIconSize} />
                </IconButton>
              </Tooltip>
              {diagnostics.showInDefinition && (
                <Tooltip
                  title={
                    showInDefinition
                      ? t('records.fieldDiagnostics.showInDefinition')
                      : t('records.fieldDiagnostics.notInDefinition')
                  }
                >
                  <span>
                    <IconButton
                      aria-label={t(
                        'records.fieldDiagnostics.showInDefinition',
                      )}
                      disabled={!showInDefinition}
                      onClick={() => {
                        afterClose.current = showInDefinition;
                        setRevealing(true);
                        setAnchor(null);
                      }}
                      size="small"
                    >
                      <CodeXmlIcon size={smallIconSize} />
                    </IconButton>
                  </span>
                </Tooltip>
              )}
            </Stack>
          </DiagnosticSection>
          <DiagnosticSection title={t('records.fieldDiagnostics.valueType')}>
            <Typography component="div" variant="body2">
              <ValueTypeLabel valueType={attribute.value_type} />
            </Typography>
            {attribute.value_type === attributeValueTypes.relationship &&
              attribute.cardinality && (
                <Typography color="text.secondary" variant="body2">
                  {attribute.cardinality === attributeCardinalities.many
                    ? t('records.fieldDiagnostics.linksToMany')
                    : t('records.fieldDiagnostics.linksToOne')}
                </Typography>
              )}
          </DiagnosticSection>
          <DiagnosticSection title={t('records.fieldDiagnostics.valueSource')}>
            {diagnostics.showsValues && (
              <Typography variant="body2">
                {isEmptyValue(resolved?.value)
                  ? t('records.fieldDiagnostics.empty')
                  : !sourceContext
                    ? t('records.fieldDiagnostics.set')
                    : sourceContext.code === contextCode
                      ? sourceContext.code === defaultContextCode
                        ? t('records.fieldDiagnostics.setInDefaultContext')
                        : t('records.fieldDiagnostics.setInContext', {
                            context: sourceContext.code,
                          })
                      : sourceContext.code === defaultContextCode
                        ? t(
                            'records.fieldDiagnostics.inheritedFromDefaultContext',
                          )
                        : t('records.fieldDiagnostics.inheritedFromContext', {
                            context: sourceContext.code,
                          })}
              </Typography>
            )}
            <Typography color="text.secondary" variant="body2">
              {fallsBack
                ? t('records.fieldDiagnostics.fallsBack')
                : t('records.fieldDiagnostics.noFallback')}{' '}
              {editableEverywhere
                ? t('records.fieldDiagnostics.editableEverywhere')
                : t('records.fieldDiagnostics.editableInDefault')}
            </Typography>
          </DiagnosticSection>
          {connections.length > 0 && (
            <DiagnosticSection
              title={t('records.fieldDiagnostics.connections')}
            >
              <FieldConnectionList
                attributeCode={attribute.code}
                attributes={diagnostics.attributes ?? []}
                connections={connections}
                onReveal={
                  revealField
                    ? (code) => {
                        setRevealing(true);
                        setAnchor(null);
                        revealField(code);
                      }
                    : undefined
                }
              />
            </DiagnosticSection>
          )}
          {editorDiagnosis && (
            <DiagnosticSection title={t('records.fieldDiagnostics.input')}>
              <EditorDiagnosis diagnosis={editorDiagnosis} />
            </DiagnosticSection>
          )}
          {diagnostics.findingsStatus === fieldFindingsStatuses.loading && (
            <DiagnosticSection title={t('records.fieldDiagnostics.findings')}>
              <Typography color="text.secondary" variant="body2">
                {t('records.fieldDiagnostics.loadingFindings')}
              </Typography>
            </DiagnosticSection>
          )}
          {diagnostics.findingsStatus === fieldFindingsStatuses.failed && (
            <DiagnosticSection title={t('records.fieldDiagnostics.findings')}>
              <Typography color="error" role="alert" variant="body2">
                {t('records.fieldDiagnostics.couldNotLoadFindings', {
                  message: errorMessage(diagnostics.findingsError) ?? '',
                })}
              </Typography>
            </DiagnosticSection>
          )}
          {findings && (
            <DiagnosticSection title={t('records.fieldDiagnostics.findings')}>
              {findings.length === 0 ? (
                <Typography color="text.secondary" variant="body2">
                  {t('records.fieldDiagnostics.noFindings')}
                </Typography>
              ) : (
                <Stack component="ul" spacing={1} sx={{ m: 0, p: 0 }}>
                  {findings.map((finding) => (
                    <Stack
                      component="li"
                      direction="row"
                      key={finding.id}
                      spacing={1}
                      sx={{ alignItems: 'flex-start', listStyle: 'none' }}
                    >
                      <FindingSeverityChip severity={finding.severity} />
                      <Typography variant="body2">
                        {findingMessage(
                          finding.message,
                          diagnostics.attributes ?? [attribute],
                        )}
                      </Typography>
                    </Stack>
                  ))}
                </Stack>
              )}
            </DiagnosticSection>
          )}
        </Stack>
      </Popover>
    </>
  );
};
