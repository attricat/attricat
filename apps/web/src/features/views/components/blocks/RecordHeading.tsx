import { Box, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageTitle } from '../../../../components/PageTitle';
import { RecordIcon } from '../../../../components/systemIcons';
import {
  viewBlockTypes,
  type Attribute,
  type ViewDefinition,
} from '../../../records/api';
import { statusConfiguration } from '../../../records/status';
import { StatusValue } from '../../controls/values';
import { FieldErrorBoundary } from '../boundaries/FieldErrorBoundary';
import { resolveValueRenderer } from '../registry';
import { attributeLabel } from '../../../records/recordDisplay';
import { AttributeValueText } from '../values/AttributeValueText';
import { findRecordHeading } from './RecordHeadingDefinition';

type ResolvedValue = { value: unknown };

export const RecordHeading = ({
  attributes,
  compact = false,
  contextId,
  recordId,
  values,
  view,
}: {
  attributes: readonly Attribute[];
  compact?: boolean;
  contextId?: string;
  recordId: string;
  values: Record<string, ResolvedValue>;
  view?: ViewDefinition;
}) => {
  const { t } = useTranslation();
  const heading = findRecordHeading(view);
  const title =
    heading?.type === viewBlockTypes.stack ? heading.children[0] : undefined;
  const titleAttribute =
    title?.type === viewBlockTypes.field
      ? attributes.find((attribute) => attribute.code === title.field)
      : undefined;
  const titleValue = titleAttribute
    ? values[titleAttribute.code]?.value
    : undefined;
  const titleProps = compact
    ? ({ component: 'h2', variant: 'h4' } as const)
    : {};
  const fallback = (
    <PageTitle icon={RecordIcon} {...titleProps}>
      {recordId}
    </PageTitle>
  );
  if (!titleAttribute || titleValue === null || titleValue === undefined)
    return fallback;
  return (
    <Box>
      <FieldErrorBoundary
        fallbackMessage={t('views.unableToRenderRecordHeading')}
        logLabel="record heading"
      >
        <PageTitle icon={RecordIcon} {...titleProps}>
          <AttributeValueText attribute={titleAttribute} value={titleValue} />
        </PageTitle>
      </FieldErrorBoundary>
      {heading?.type === viewBlockTypes.stack &&
        heading.children.slice(1).map((child, index) => {
          if (child.type === viewBlockTypes.text)
            return (
              <Typography color="text.secondary" key={index}>
                {child.text}
              </Typography>
            );
          if (child.type === viewBlockTypes.field) {
            const attribute = attributes.find(
              (item) => item.code === child.field,
            );
            const value = attribute ? values[attribute.code]?.value : undefined;
            if (!attribute || value === undefined) return null;
            // Fields with their own read-only control render as they do in
            // the view body; everything else stays secondary heading text.
            const Renderer = resolveValueRenderer(child.component);
            if (Renderer)
              return (
                <FieldErrorBoundary
                  fallbackMessage={t('views.unableToRenderAttribute', {
                    attribute: attributeLabel(attribute),
                  })}
                  key={index}
                  logLabel={attributeLabel(attribute)}
                >
                  <Renderer
                    attribute={attribute}
                    component={child.component}
                    contextId={contextId}
                    recordId={recordId}
                    value={value}
                  />
                </FieldErrorBoundary>
              );
            const status = statusConfiguration(attribute);
            if (status)
              return <StatusValue config={status} key={index} value={value} />;
            return (
              <Typography color="text.secondary" key={index}>
                <AttributeValueText attribute={attribute} value={value} />
              </Typography>
            );
          }
          return null;
        })}
    </Box>
  );
};
