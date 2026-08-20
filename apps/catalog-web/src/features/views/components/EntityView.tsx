import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Divider,
  Paper,
  Stack,
  Tab,
  Tabs,
  Typography,
} from '@mui/material';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import { createElement, useState, type ReactNode } from 'react';
import {
  viewBlockTypes,
  type Attribute,
  type ComponentReference,
  type ViewDefinition,
  type ViewNode,
} from '../../entities/api';
import { FieldErrorBoundary } from './boundaries/FieldErrorBoundary';
import {
  resolveIncomingRelationshipRenderer,
  resolveValueRenderer,
  resolveViewComponent,
} from './registry';
import { AttributeValue } from './values/AttributeValue';
import { IncomingRelationshipListDisplay } from './IncomingRelationshipListDisplay';

type ResolvedValue = { value: unknown; source_context?: { code: string } };
type Props = {
  view?: ViewDefinition;
  attributes: readonly Attribute[];
  values: Record<string, ResolvedValue>;
  renderEditor?: (attribute: Attribute) => ReactNode;
  skipComponentId?: string;
  contextId?: string;
  entityId?: string;
};

const labelFor = (field: string) => field.replaceAll('_', ' ');

const ValueField = ({
  attribute,
  resolved,
  renderEditor,
  component,
  contextId,
  entityId,
}: {
  attribute: Attribute;
  resolved?: ResolvedValue;
  renderEditor?: (attribute: Attribute) => ReactNode;
  component?: ComponentReference | null;
  contextId?: string;
  entityId?: string;
}) => (
  <FieldErrorBoundary label={labelFor(attribute.code)}>
    <Stack spacing={0.5}>
      {renderEditor ? (
        renderEditor(attribute)
      ) : (
        <>
          <Typography sx={{ fontWeight: 700 }} variant="subtitle2">
            {labelFor(attribute.code)}
          </Typography>
          {(() => {
            const ValueRenderer =
              resolveValueRenderer(component) ?? AttributeValue;
            return (
              <ValueRenderer
                attribute={attribute}
                component={component}
                contextId={contextId}
                entityId={entityId}
                value={resolved?.value}
              />
            );
          })()}
          {resolved?.source_context && (
            <Typography color="text.secondary" variant="caption">
              Using {resolved.source_context.code}
            </Typography>
          )}
        </>
      )}
    </Stack>
  </FieldErrorBoundary>
);

const ViewTabs = ({
  tabs,
  render,
}: {
  tabs: { label: string; children: ViewNode[] }[];
  render: (nodes: ViewNode[]) => ReactNode;
}) => {
  const [value, setValue] = useState(0);
  return (
    <>
      <Tabs
        onChange={(_, next) => setValue(next)}
        value={value}
        variant="scrollable"
      >
        {tabs.map((tab) => (
          <Tab key={tab.label} label={tab.label} />
        ))}
      </Tabs>
      {tabs[value] && <Box sx={{ pt: 2 }}>{render(tabs[value].children)}</Box>}
    </>
  );
};

export const EntityView = ({
  view,
  attributes,
  values,
  renderEditor,
  skipComponentId,
  contextId,
  entityId,
}: Props) => {
  const byCode = new Map(
    attributes.map((attribute) => [attribute.code, attribute]),
  );
  const fallback: ViewNode[] = attributes.map((attribute) => ({
    type: viewBlockTypes.field,
    field: attribute.code,
  }));
  const renderNodes = (nodes: ViewNode[]): ReactNode => (
    <Stack spacing={2}>
      {nodes.map((node, index) => renderNode(node, `${node.type}-${index}`))}
    </Stack>
  );
  const renderNode = (node: ViewNode, key: string): ReactNode => {
    if (node.component && !resolveViewComponent(node.component)) {
      return (
        <FieldErrorBoundary key={key} label="view component">
          Unable to render component.
        </FieldErrorBoundary>
      );
    }
    if (skipComponentId && node.component?.id === skipComponentId) return null;
    if (node.type === viewBlockTypes.heading)
      return (
        <Typography component="h2" key={key} variant="h5">
          {node.text}
        </Typography>
      );
    if (node.type === viewBlockTypes.text)
      return (
        <Typography color="text.secondary" key={key}>
          {node.text}
        </Typography>
      );
    if (node.type === viewBlockTypes.divider) return <Divider key={key} />;
    if (node.type === viewBlockTypes.grid)
      return (
        <Box
          key={key}
          sx={{
            display: 'grid',
            gap: 2,
            gridTemplateColumns: { xs: '1fr', md: 'repeat(2, minmax(0, 1fr))' },
          }}
        >
          {node.children.map((child, index) =>
            renderNode(child, `${key}-${index}`),
          )}
        </Box>
      );
    if (node.type === viewBlockTypes.section)
      return (
        <Paper key={key} sx={{ p: 2.5 }}>
          {renderNodes(node.children)}
        </Paper>
      );
    if (node.type === viewBlockTypes.tabs)
      return (
        <Box key={key}>
          <ViewTabs render={renderNodes} tabs={node.tabs} />
        </Box>
      );
    if (node.type === viewBlockTypes.accordion)
      return (
        <Box key={key}>
          {node.sections.map((section) => (
            <Accordion key={section.label}>
              <AccordionSummary expandIcon={<ExpandMoreIcon />}>
                <Typography>{section.label}</Typography>
              </AccordionSummary>
              <AccordionDetails>
                {renderNodes(section.children)}
              </AccordionDetails>
            </Accordion>
          ))}
        </Box>
      );
    if (node.type === viewBlockTypes.stack)
      return (
        <Stack key={key} spacing={2}>
          {node.children.map((child, index) =>
            renderNode(child, `${key}-${index}`),
          )}
        </Stack>
      );
    if (node.type === viewBlockTypes.incomingRelationshipList) {
      if (!entityId)
        return (
          <Typography color="text.secondary" key={key}>
            {node.label} is available on entity previews.
          </Typography>
        );
      const Renderer =
        resolveIncomingRelationshipRenderer(node.component) ??
        IncomingRelationshipListDisplay;
      return createElement(Renderer, { entityId, key, node });
    }
    if (
      node.type === viewBlockTypes.field ||
      node.type === viewBlockTypes.relationshipList
    ) {
      const attribute = byCode.get(node.field);
      return attribute ? (
        <ValueField
          attribute={attribute}
          component={node.component}
          contextId={contextId}
          entityId={entityId}
          key={key}
          renderEditor={renderEditor}
          resolved={values[attribute.code]}
        />
      ) : null;
    }
    return null;
  };

  if (!view || view.type === viewBlockTypes.table)
    return <>{renderNodes(fallback)}</>;

  return <>{renderNode(view, 'root')}</>;
};
