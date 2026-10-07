import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Divider,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { ChevronDownIcon } from 'lucide-react';
import { createElement, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
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
  resolveViewComponent,
} from './registry';
import { IncomingRelationshipListDisplay } from './IncomingRelationshipListDisplay';
import {
  isHiddenByDefault,
  type AttributeVisibilityScope,
} from '../../entities/attributeVisibility';
import {
  ROOT_NODE_KEY,
  VIEW_COMPONENT_LOG_LABEL,
  VIEW_GRID_COLUMNS,
  VIEW_EDIT_LAYOUT_SPACING,
  VIEW_LAYOUT_SPACING,
  VIEW_SECTION_PADDING,
} from '../constants';
import { ValueField, type ResolvedValue } from './ValueField';
import { ViewTabs } from './ViewTabs';
import { lexiconText } from '../../lexicon/lexicon';

export type EntityViewProps = {
  view?: ViewDefinition;
  attributes: readonly Attribute[];
  values: Record<string, ResolvedValue>;
  renderEditor?: (
    attribute: Attribute,
    component?: ComponentReference | null,
  ) => ReactNode;
  renderAttributeDecoration?: (attribute: Attribute) => ReactNode;
  renderAttributePanel?: (attribute: Attribute) => ReactNode;
  renderFilePanel?: (attribute: Attribute, fileId: string) => ReactNode;
  skipComponentId?: string;
  contextId?: string;
  entityId?: string;
  fallbackVisibilityScope?: AttributeVisibilityScope;
  /** Lays grids out in one column regardless of the viewport, as in a narrow panel. */
  singleColumn?: boolean;
};

export const EntityView = ({
  view,
  attributes,
  values,
  renderEditor,
  renderAttributeDecoration,
  renderAttributePanel,
  renderFilePanel,
  skipComponentId,
  contextId,
  entityId,
  fallbackVisibilityScope,
  singleColumn = false,
}: EntityViewProps) => {
  const { t } = useTranslation();
  const spacing = renderEditor ? VIEW_EDIT_LAYOUT_SPACING : VIEW_LAYOUT_SPACING;
  const byCode = new Map(
    attributes.map((attribute) => [attribute.code, attribute]),
  );
  const fallback: ViewNode[] = attributes
    .filter(
      (attribute) =>
        !fallbackVisibilityScope ||
        !isHiddenByDefault(attribute, fallbackVisibilityScope),
    )
    .map((attribute) => ({
      type: viewBlockTypes.field,
      field: attribute.code,
    }));
  const renderNodes = (nodes: ViewNode[]): ReactNode => (
    <Stack spacing={spacing}>
      {nodes.map((node, index) => renderNode(node, `${node.type}-${index}`))}
    </Stack>
  );
  const renderNode = (node: ViewNode, key: string): ReactNode => {
    if (node.component && !resolveViewComponent(node.component)) {
      return (
        <FieldErrorBoundary
          fallbackMessage={t('views.unableToRenderComponent')}
          key={key}
          logLabel={VIEW_COMPONENT_LOG_LABEL}
        >
          {t('views.unableToRenderComponent')}
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
            gap: spacing,
            gridTemplateColumns: singleColumn ? '1fr' : VIEW_GRID_COLUMNS,
          }}
        >
          {node.children.map((child, index) =>
            renderNode(child, `${key}-${index}`),
          )}
        </Box>
      );
    if (node.type === viewBlockTypes.section)
      return (
        <Paper key={key} sx={{ p: VIEW_SECTION_PADDING }}>
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
              <AccordionSummary expandIcon={<ChevronDownIcon />}>
                <Typography>{lexiconText(section.label)}</Typography>
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
        <Stack key={key} spacing={spacing}>
          {node.children.map((child, index) =>
            renderNode(child, `${key}-${index}`),
          )}
        </Stack>
      );
    if (node.type === viewBlockTypes.incomingRelationshipList) {
      if (!entityId)
        return (
          <Typography color="text.secondary" key={key}>
            {t('views.availableOnPreviews', {
              label: lexiconText(node.label),
            })}
          </Typography>
        );
      const Renderer =
        resolveIncomingRelationshipRenderer(node.component) ??
        IncomingRelationshipListDisplay;
      // Registered renderers receive the label already resolved.
      return createElement(Renderer, {
        entityId,
        key,
        node: { ...node, label: lexiconText(node.label) },
      });
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
          renderAttributeDecoration={renderAttributeDecoration}
          renderAttributePanel={renderAttributePanel}
          renderFilePanel={renderFilePanel}
          renderEditor={renderEditor}
          resolved={values[attribute.code]}
        />
      ) : null;
    }
    return null;
  };

  if (
    !view ||
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.extensionLayout
  )
    return <>{renderNodes(fallback)}</>;

  return <>{renderNode(view, ROOT_NODE_KEY)}</>;
};
