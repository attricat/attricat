import { Alert, Box, Divider, Paper, Stack, Typography } from '@mui/material';
import { createElement, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  viewBlockTypes,
  type Attribute,
  type ComponentReference,
  type ViewDefinition,
  type ViewNode,
} from '../../records/api';
import {
  resolveIncomingRelationshipRenderer,
  resolveViewComponent,
} from './registry';
import { IncomingRelationshipListDisplay } from './IncomingRelationshipListDisplay';
import {
  isHiddenByDefault,
  type AttributeVisibilityScope,
} from '../../records/attributeVisibility';
import {
  ROOT_NODE_KEY,
  VIEW_GRID_COLUMNS,
  VIEW_EDIT_LAYOUT_SPACING,
  VIEW_EDIT_TAB_PANEL_SPACING,
  VIEW_TAB_PANEL_SPACING,
  VIEW_LAYOUT_SPACING,
  VIEW_SECTION_PADDING,
} from '../constants';
import { ValueField, type ResolvedValue } from './ValueField';
import { ViewTabs } from './ViewTabs';
import { ViewAccordionSection } from './ViewAccordionSection';
import { lexiconText } from '../../lexicon/lexicon';

export type RecordViewProps = {
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
  recordId?: string;
  fallbackVisibilityScope?: AttributeVisibilityScope;
  /** Fields with errors, so collapsed layout blocks can point to them. */
  invalidFields?: ReadonlySet<string>;
  /** Lays grids out in one column regardless of the viewport, as in a narrow panel. */
  singleColumn?: boolean;
};

export const RecordView = ({
  view,
  attributes,
  values,
  renderEditor,
  renderAttributeDecoration,
  renderAttributePanel,
  renderFilePanel,
  skipComponentId,
  contextId,
  recordId,
  fallbackVisibilityScope,
  invalidFields,
  singleColumn = false,
}: RecordViewProps) => {
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
        <Alert key={key} severity="warning">
          {t('views.unableToRenderComponent')}
        </Alert>
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
          <ViewTabs
            invalidFields={invalidFields}
            panelSpacing={
              renderEditor
                ? VIEW_EDIT_TAB_PANEL_SPACING
                : VIEW_TAB_PANEL_SPACING
            }
            render={renderNodes}
            tabs={node.tabs}
          />
        </Box>
      );
    if (node.type === viewBlockTypes.accordion)
      return (
        <Box key={key}>
          {node.sections.map((section) => (
            <ViewAccordionSection
              invalidFields={invalidFields}
              key={section.label}
              label={section.label}
              nodes={section.children}
            >
              {renderNodes(section.children)}
            </ViewAccordionSection>
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
      if (!recordId)
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
        recordId,
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
          recordId={recordId}
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
    view.type === viewBlockTypes.pageTitle ||
    view.type === viewBlockTypes.extensionLayout ||
    view.type === viewBlockTypes.explorer
  )
    return <>{renderNodes(fallback)}</>;

  return <>{renderNode(view, ROOT_NODE_KEY)}</>;
};
