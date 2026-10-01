import { Paper, Tab, Tabs, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import type { Attribute, Blueprint, ViewDefinition } from '../entities/api';
import { EntityView } from '../views/components/EntityView';
import { editViewName } from './constants';
import { RenderedBlueprintView } from './RenderedBlueprintView';
import { SandboxAttributeEditor } from './SandboxAttributeEditor';
import { sandboxValuesForFields } from './sandboxValues';

type ViewTab = readonly [name: string, view: ViewDefinition | undefined];

/** The edit view always comes first, even when the blueprint omits it. */
const viewTabs = (views: Blueprint['views']): ViewTab[] => {
  const entries = Object.entries(views);
  const editEntry = entries.find(([name]) => name === editViewName);
  const previewEntries = entries.filter(([name]) => name !== editViewName);
  return [editEntry ?? [editViewName, undefined], ...previewEntries];
};

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
          <Tab {...tabId.tab(index)} key={name} label={name} value={name} />
        ))}
      </Tabs>
      <Paper
        {...tabId.panel(activeViewIndex)}
        sx={{ mt: 2, p: 2.5 }}
        variant="outlined"
      >
        {activeViewName === editViewName ? (
          <EntityView
            attributes={attributes}
            fallbackVisibilityScope="form"
            renderEditor={(attribute, component) => (
              <SandboxAttributeEditor
                attribute={attribute}
                component={component}
                onChange={(value) => updateField(attribute.code, value)}
                value={fields[attribute.code] ?? ''}
              />
            )}
            values={values}
            view={activeView}
          />
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
