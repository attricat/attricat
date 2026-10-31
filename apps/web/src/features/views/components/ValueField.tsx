import { Box, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { defaultContextCode } from '../../contexts/constants';
import { IconLabel } from '../../../components/IconLabel';
import { FieldNotesRow } from '../../extensions/fieldNotes';
import { RelationshipIcon } from '../../../components/systemIcons';
import { fileAttributeIcon } from '../../files/fileAttributeIcon';
import type {
  Attribute,
  ComponentReference,
  EditorReference,
} from '../../records/api';
import { FieldDiagnosticsButton } from '../../records/components/FieldDiagnosticsButton';
import {
  useFieldDiagnostics,
  useFieldFindings,
  worstSeverity,
} from '../../records/fieldDiagnostics';
import { findingSeverityColor } from '../../rules/findingSeverity';
import { attributeLabel } from '../../records/recordDisplay';
import { attributeValueTypes } from '../../records/valueTypes';
import { FieldErrorBoundary } from './boundaries/FieldErrorBoundary';
import { resolveValueRenderer } from './registry';
import { AttributeValue } from './values/AttributeValue';

/** Room between a field and the edge of the tint that marks its findings. */
const fieldFindingTintInset = 1.5;

export type ResolvedValue = {
  value: unknown;
  source_context?: { id: string; code: string };
};

export const ValueField = ({
  attribute,
  resolved,
  renderEditor,
  component,
  editor: editorReference,
  contextId,
  recordId,
}: {
  attribute: Attribute;
  resolved?: ResolvedValue;
  renderEditor?: (
    attribute: Attribute,
    component?: ComponentReference | null,
    editor?: EditorReference | null,
  ) => ReactNode;
  component?: ComponentReference | null;
  /** An extension's attribute editor the view names for this field. */
  editor?: EditorReference | null;
  contextId?: string;
  recordId?: string;
}) => {
  const { t } = useTranslation();
  const label = attributeLabel(attribute);
  // File and relationship labels lead with what the field holds.
  const labelIcon =
    attribute.value_type === attributeValueTypes.file
      ? fileAttributeIcon(attribute)
      : attribute.value_type === attributeValueTypes.relationship
        ? RelationshipIcon
        : undefined;
  // An editor host returns nothing for a value it does not let the user edit.
  const editor = renderEditor?.(attribute, component, editorReference) ?? null;
  const diagnostics = useFieldDiagnostics();
  const findings = useFieldFindings(attribute.code);
  const severity = findings && worstSeverity(findings);
  // The diagnostics button keeps beside an editor, clear of its outline,
  // label and save status.
  const editorField =
    editor !== null && diagnostics ? (
      <Box sx={{ alignItems: 'flex-start', display: 'flex', gap: 0.5 }}>
        <Box sx={{ flex: 1, minWidth: 0 }}>{editor}</Box>
        <Box sx={{ flexShrink: 0, pt: 0.25 }}>
          <FieldDiagnosticsButton
            attribute={attribute}
            editor={editorReference}
            recordId={recordId}
            resolved={resolved}
          />
        </Box>
      </Box>
    ) : (
      editor
    );
  return (
    <FieldErrorBoundary
      fallbackMessage={t('views.unableToRenderAttribute', { attribute: label })}
      logLabel={label}
    >
      <Stack
        // An editor marks its own field.
        data-field-code={editor === null ? attribute.code : undefined}
        spacing={0.5}
        sx={
          // The tint reaches past the field by as much as it pads it, so
          // marking a field with data quality findings does not move the
          // layout.
          severity
            ? {
                bgcolor: `${findingSeverityColor(severity)}.tint`,
                borderRadius: 1,
                m: -fieldFindingTintInset,
                p: fieldFindingTintInset,
              }
            : undefined
        }
      >
        {editorField !== null ? (
          editorField
        ) : (
          <>
            <Box sx={{ alignItems: 'center', display: 'flex', gap: 0.5 }}>
              <Typography sx={{ fontWeight: 700 }} variant="subtitle2">
                {labelIcon ? (
                  <IconLabel icon={labelIcon}>{label}</IconLabel>
                ) : (
                  label
                )}
              </Typography>
              <FieldDiagnosticsButton
                attribute={attribute}
                editor={editorReference}
                recordId={recordId}
                resolved={resolved}
              />
            </Box>
            {(() => {
              const ValueRenderer =
                resolveValueRenderer(component) ?? AttributeValue;
              return (
                <ValueRenderer
                  attribute={attribute}
                  component={component}
                  contextId={contextId}
                  recordId={recordId}
                  value={resolved?.value}
                />
              );
            })()}
            {resolved?.source_context &&
              contextId &&
              resolved.source_context.id !== contextId && (
                <Typography color="text.secondary" variant="caption">
                  {resolved.source_context.code === defaultContextCode
                    ? t('views.inheritedFromDefaultContext')
                    : t('views.inheritedFromContext', {
                        context: resolved.source_context.code,
                      })}
                </Typography>
              )}
          </>
        )}
        <FieldNotesRow attributeCode={attribute.code} />
      </Stack>
    </FieldErrorBoundary>
  );
};
