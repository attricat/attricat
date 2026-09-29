import { Box, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute, ComponentReference } from '../../entities/api';
import { attributeLabel } from '../../entities/entityDisplay';
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
  entityId,
}: {
  attribute: Attribute;
  resolved?: ResolvedValue;
  renderEditor?: (attribute: Attribute) => ReactNode;
  renderAttributeDecoration?: (attribute: Attribute) => ReactNode;
  renderAttributePanel?: (attribute: Attribute) => ReactNode;
  renderFilePanel?: (attribute: Attribute, fileId: string) => ReactNode;
  component?: ComponentReference | null;
  contextId?: string;
  entityId?: string;
}) => {
  const { t } = useTranslation();
  const label = attributeLabel(attribute);
  return (
    <FieldErrorBoundary
      fallbackMessage={t('views.unableToRenderAttribute', { attribute: label })}
      logLabel={label}
    >
      <Stack spacing={0.5}>
        {renderEditor ? (
          renderEditor(attribute)
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
                  entityId={entityId}
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
        {renderEditor && renderAttributeDecoration?.(attribute)}
        {renderAttributePanel?.(attribute)}
      </Stack>
    </FieldErrorBoundary>
  );
};
