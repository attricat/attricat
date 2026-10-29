import { Box, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { defaultContextCode } from '../../contexts/constants';
import { IconLabel } from '../../../components/IconLabel';
import { compactIconButtonSize } from '../../../components/iconSizes';
import { RelationshipIcon } from '../../../components/systemIcons';
import { fileAttributeIcon } from '../../files/fileAttributeIcon';
import type { Attribute, ComponentReference } from '../../records/api';
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
  renderAttributeDecoration,
  renderAttributePanel,
  renderFilePanel,
  component,
  contextId,
  recordId,
}: {
  attribute: Attribute;
  resolved?: ResolvedValue;
  renderEditor?: (
    attribute: Attribute,
    component?: ComponentReference | null,
  ) => ReactNode;
  renderAttributeDecoration?: (attribute: Attribute) => ReactNode;
  renderAttributePanel?: (attribute: Attribute) => ReactNode;
  renderFilePanel?: (attribute: Attribute, fileId: string) => ReactNode;
  component?: ComponentReference | null;
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
  const editor = renderEditor?.(attribute, component) ?? null;
  const diagnostics = useFieldDiagnostics();
  const findings = useFieldFindings(attribute.code);
  const severity = findings && worstSeverity(findings);
  // An extension decoration renders its trigger only once it has content, so
  // its slot keeps a fixed size and the field never moves when it appears.
  const decorationSlot = renderAttributeDecoration && (
    <Box
      sx={{
        alignItems: 'center',
        display: 'flex',
        flexShrink: 0,
        height: compactIconButtonSize,
        width: compactIconButtonSize,
      }}
    >
      {renderAttributeDecoration(attribute)}
    </Box>
  );
  // The diagnostics button and decoration keep beside an editor, clear of its
  // outline, label and save status.
  const editorField =
    editor !== null && (diagnostics || decorationSlot) ? (
      <Box sx={{ alignItems: 'flex-start', display: 'flex', gap: 0.5 }}>
        <Box sx={{ flex: 1, minWidth: 0 }}>{editor}</Box>
        <Box sx={{ display: 'flex', flexShrink: 0, pt: 0.25 }}>
          <FieldDiagnosticsButton attribute={attribute} resolved={resolved} />
          {decorationSlot}
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
                resolved={resolved}
              />
              {decorationSlot}
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
                  renderFilePanel={
                    renderFilePanel &&
                    attribute.value_type === attributeValueTypes.file
                      ? (fileId) => renderFilePanel(attribute, fileId)
                      : undefined
                  }
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
        {renderAttributePanel?.(attribute)}
      </Stack>
    </FieldErrorBoundary>
  );
};
