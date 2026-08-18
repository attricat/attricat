import { Box, Typography } from '@mui/material';
import type { Attribute, ViewDefinition, ViewNode } from '../../../entities/api';
import { FieldErrorBoundary } from '../boundaries/FieldErrorBoundary';
import { formatAttributeValue } from '../values/AttributeValue';

type ResolvedValue = { value: unknown };

export const findEntityHeading = (view: ViewDefinition | undefined): ViewNode | undefined => {
  if (!view || view.type === 'table') return undefined;
  const visit = (node: ViewNode): ViewNode | undefined => {
    if (node.type === 'stack' && node.component?.id === 'catalog.entity_heading') return node;
    if ('children' in node) return node.children.map(visit).find(Boolean);
    if (node.type === 'tabs') return node.tabs.flatMap((tab) => tab.children).map(visit).find(Boolean);
    if (node.type === 'accordion') return node.sections.flatMap((section) => section.children).map(visit).find(Boolean);
    return undefined;
  };
  return visit(view);
};

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
  const titleAttribute = title?.type === 'field'
    ? attributes.find((attribute) => attribute.code === title.field)
    : undefined;
  const titleValue = titleAttribute ? values[titleAttribute.code]?.value : undefined;
  const fallback = <Typography component="h1" variant="h3">{entityId}</Typography>;
  if (!titleAttribute || titleValue === null || titleValue === undefined) return fallback;
  return (
    <Box>
      <FieldErrorBoundary label="entity heading">
        <Typography component="h1" variant="h3">
          {formatAttributeValue(titleAttribute, titleValue)}
        </Typography>
      </FieldErrorBoundary>
      {heading?.type === 'stack' && heading.children.slice(1).map((child, index) => {
        if (child.type === 'text') return <Typography color="text.secondary" key={index}>{child.text}</Typography>;
        if (child.type === 'field') {
          const attribute = attributes.find((item) => item.code === child.field);
          const value = attribute ? values[attribute.code]?.value : undefined;
          return attribute && value !== undefined ? <Typography color="text.secondary" key={index}>{formatAttributeValue(attribute, value)}</Typography> : null;
        }
        return null;
      })}
    </Box>
  );
};
