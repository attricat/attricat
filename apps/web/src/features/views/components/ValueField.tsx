import { Box, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute, ComponentReference } from '../../records/api';
import { attributeLabel } from '../../records/recordDisplay';
import { FieldErrorBoundary } from './boundaries/FieldErrorBoundary';
import { resolveValueRenderer } from './registry';
import { AttributeValue } from './values/AttributeValue';

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
  // An editor host returns nothing for a value it does not let the user edit.
  const editor = renderEditor?.(attribute, component) ?? null;
  return (
    <FieldErrorBoundary
      fallbackMessage={t('views.unableToRenderAttribute', { attribute: label })}
      logLabel={label}
    >
      <Stack spacing={0.5}>
        {editor !== null ? (
          editor
        ) : (
          <>
            <Box sx={{ alignItems: 'center', display: 'flex', gap: 0.5 }}>
              <Typography sx={{ fontWeight: 700 }} variant="subtitle2">
                {attributeLabel(attribute)}
              </Typography>
              {renderAttributeDecoration?.(attribute)}
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
                    renderFilePanel && attribute.value_type === 'file'
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
                  {t('views.inheritedFromContext', {
                    context: resolved.source_context.code,
                  })}
                </Typography>
              )}
          </>
        )}
        {editor !== null && renderAttributeDecoration?.(attribute)}
        {renderAttributePanel?.(attribute)}
      </Stack>
    </FieldErrorBoundary>
  );
};
