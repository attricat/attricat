import { Box, Typography } from '@mui/material';
import {
  viewBlockTypes,
  type Attribute,
  type ViewDefinition,
} from '../../../entities/api';
import { FieldErrorBoundary } from '../boundaries/FieldErrorBoundary';
import { formatAttributeValue } from '../values/format-attribute-value';
import { findEntityHeading } from './EntityHeadingDefinition';

type ResolvedValue = { value: unknown };

export const EntityHeading = ({
  attributes,
  entityId,
  values,
  view,
}: {
  attributes: readonly Attribute[];
  entityId: string;
  values: Record<string, ResolvedValue>;
  view?: ViewDefinition;
}) => {
  const heading = findEntityHeading(view);
  const title = heading?.type === 'stack' ? heading.children[0] : undefined;
  const titleAttribute =
    title?.type === viewBlockTypes.field
      ? attributes.find((attribute) => attribute.code === title.field)
      : undefined;
  const titleValue = titleAttribute
    ? values[titleAttribute.code]?.value
    : undefined;
  const fallback = (
    <Typography component="h1" variant="h3">
      {entityId}
    </Typography>
  );
  if (!titleAttribute || titleValue === null || titleValue === undefined)
    return fallback;
  return (
    <Box>
      <FieldErrorBoundary label="entity heading">
        <Typography component="h1" variant="h3">
          {formatAttributeValue(titleAttribute, titleValue)}
        </Typography>
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
                {formatAttributeValue(attribute, value)}
              </Typography>
            ) : null;
          }
          return null;
        })}
    </Box>
  );
};
