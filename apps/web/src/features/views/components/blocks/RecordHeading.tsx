import { Box, Typography } from '@mui/material';
import i18n from '../../../../i18n';
import { PageTitle } from '../../../../components/PageTitle';
import { RecordIcon } from '../../../../components/systemIcons';
import {
  viewBlockTypes,
  type Attribute,
  type ViewDefinition,
} from '../../../records/api';
import { FieldErrorBoundary } from '../boundaries/FieldErrorBoundary';
import { AttributeValueText } from '../values/AttributeValueText';
import { findRecordHeading } from './RecordHeadingDefinition';

type ResolvedValue = { value: unknown };

export const RecordHeading = ({
  attributes,
  compact = false,
  recordId,
  values,
  view,
}: {
  attributes: readonly Attribute[];
  compact?: boolean;
  recordId: string;
  values: Record<string, ResolvedValue>;
  view?: ViewDefinition;
}) => {
  const heading = findRecordHeading(view);
  const title = heading?.type === 'stack' ? heading.children[0] : undefined;
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
        fallbackMessage={i18n.t('views.unableToRenderRecordHeading')}
        logLabel="record heading"
      >
        <PageTitle icon={RecordIcon} {...titleProps}>
          <AttributeValueText attribute={titleAttribute} value={titleValue} />
        </PageTitle>
      </FieldErrorBoundary>
      {heading?.type === 'stack' &&
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
            return attribute && value !== undefined ? (
              <Typography color="text.secondary" key={index}>
                <AttributeValueText attribute={attribute} value={value} />
              </Typography>
            ) : null;
          }
          return null;
        })}
    </Box>
  );
};
