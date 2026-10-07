import { VIEW_EDIT_LAYOUT_SPACING } from '../views/constants';
import { Box, Paper, Tab, Tabs, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import type {
  Attribute,
  Blueprint,
  ComponentReference,
  ViewDefinition,
} from '../entities/api';
import { EntityView } from '../views/components/EntityView';
import { entityHeadingComponentId } from '../views/components/blocks/EntityHeadingDefinition';
import { resolveEditComponent } from '../views/components/registry';
import {
  headingEditableAttributes,
  unplacedEditableAttributes,
} from '../entities/entityFormAttributes';
import { legacyEditViewName, sandboxEditTab } from './constants';
import { RenderedBlueprintView } from './RenderedBlueprintView';
import { SandboxAttributeEditor } from './SandboxAttributeEditor';
import { sandboxValuesForFields } from './sandboxValues';

type ViewTab = readonly [name: string, view: ViewDefinition | undefined];

/**
 * The sandbox editor, using the detail layout, always comes first. A
 * deprecated `edit` view is not shown.
 */
const viewTabs = (views: Blueprint['views']): ViewTab[] => [
  [sandboxEditTab, views.detail],
  ...Object.entries(views).filter(([name]) => name !== legacyEditViewName),
];

export const BlueprintViewsPreview = ({
  attributes,
  views,
}: {
  attributes: readonly Attribute[];
  views: Blueprint['views'];
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  const [fields, setFields] = useState<Record<string, string>>({});
  const tabs = viewTabs(views);
  const [selectedView, setSelectedView] = useState(tabs[0][0]);
  const activeViewIndex = Math.max(
    0,
    tabs.findIndex(([name]) => name === selectedView),
  );
  const [activeViewName, activeView] = tabs[activeViewIndex];
  const values = sandboxValuesForFields(attributes, fields);
  const updateField = (code: string, value: string) =>
    setFields((current) => ({ ...current, [code]: value }));
  // Display components on the detail layout edit with their paired editor.
  const renderEditor = (
    attribute: Attribute,
    component?: ComponentReference | null,
  ) => (
    <SandboxAttributeEditor
      attribute={attribute}
      component={resolveEditComponent(component) ?? component}
      onChange={(value) => updateField(attribute.code, value)}
      value={fields[attribute.code] ?? ''}
    />
  );
  const editable = attributes.filter((attribute) => !attribute.readonly);
  const headingFields = headingEditableAttributes(editable, views.detail);
  const unplaced = unplacedEditableAttributes(editable, views.detail);

  return (
    <>
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        {t('blueprints.sandboxDescription')}
      </Typography>
      <Tabs
        allowScrollButtonsMobile
        onChange={(_, value: string) => setSelectedView(value)}
        scrollButtons="auto"
        sx={{ mt: 1 }}
        value={activeViewName}
        variant="scrollable"
      >
        {tabs.map(([name], index) => (
          <Tab
            {...tabId.tab(index)}
            key={name}
            label={name === sandboxEditTab ? t('blueprints.sandboxEdit') : name}
            value={name}
          />
        ))}
      </Tabs>
      <Paper
        {...tabId.panel(activeViewIndex)}
        sx={{ mt: 2, p: 2.5 }}
        variant="outlined"
      >
        {activeViewName === sandboxEditTab ? (
          <>
            {headingFields.length > 0 && (
              <Box sx={{ mb: VIEW_EDIT_LAYOUT_SPACING }}>
                <EntityView
                  attributes={headingFields}
                  renderEditor={renderEditor}
                  values={values}
                />
              </Box>
            )}
            <EntityView
              attributes={attributes}
              fallbackVisibilityScope="detail"
              renderEditor={renderEditor}
              skipComponentId={entityHeadingComponentId}
              values={values}
              view={activeView}
            />
            {unplaced.length > 0 && (
              <>
                <Typography component="h3" sx={{ mb: 2, mt: 3 }} variant="h6">
                  {t('entities.otherAttributes')}
                </Typography>
                <EntityView
                  attributes={unplaced}
                  renderEditor={renderEditor}
                  values={values}
                />
              </>
            )}
          </>
        ) : (
          activeView && (
            <RenderedBlueprintView
              attributes={attributes}
              values={values}
              view={activeView}
            />
          )
        )}
      </Paper>
    </>
  );
};
