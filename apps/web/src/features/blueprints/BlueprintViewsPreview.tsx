import { Alert, Box, Paper, Tab, Tabs, Typography } from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import type {
  Attribute,
  Blueprint,
  ComponentReference,
  JsonSchema,
  ViewDefinition,
} from '../records/api';
import { FieldDiagnosticsContext } from '../records/fieldDiagnostics';
import { useBlueprintFieldDiagnostics } from '../records/useBlueprintFieldDiagnostics';
import { EditableRecordLayout } from '../views/components/EditableRecordLayout';
import {
  FieldRevealContext,
  RevealFieldContext,
  useFieldReveal,
} from '../views/fieldReveal';
import { resolveEditComponent } from '../views/components/registry';
import {
  headingEditableAttributes,
  unplacedEditableAttributes,
} from '../records/recordFormAttributes';
import { sandboxEditTab } from './constants';
import { RenderedBlueprintView } from './RenderedBlueprintView';
import { SandboxAttributeEditor } from './SandboxAttributeEditor';
import { sandboxValidation, sandboxValuesForFields } from './sandboxValues';

type ViewTab = readonly [name: string, view: ViewDefinition | undefined];

/** The sandbox editor, using the detail layout, always comes first. */
const viewTabs = (views: Blueprint['views']): ViewTab[] => [
  [sandboxEditTab, views.detail],
  ...Object.entries(views),
];

export const BlueprintViewsPreview = ({
  attributes,
  definition,
  recordSchema,
  revision,
  views,
}: {
  attributes: readonly Attribute[];
  /** The definition source, for what only it records, such as unique keys. */
  definition?: string;
  recordSchema?: JsonSchema | null;
  /** The saved revision shown, whose enabled data quality rules apply. */
  revision?: { id: string; version: number };
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
  const validation = sandboxValidation(attributes, fields, recordSchema);
  const diagnostics = useBlueprintFieldDiagnostics({
    attributes,
    recordSchema,
    definition,
    revision,
  });
  const fieldsRef = useRef<HTMLDivElement>(null);
  const { request: revealRequest, reveal: revealField } =
    useFieldReveal(fieldsRef);
  const updateField = (code: string, value: string) =>
    setFields((current) => ({ ...current, [code]: value }));
  // Display components on the detail layout edit with their paired editor.
  const renderEditor = (
    attribute: Attribute,
    component?: ComponentReference | null,
  ) => (
    // Marks where the field is, to lead to it from a connected field.
    <Box data-field-code={attribute.code}>
      <SandboxAttributeEditor
        attribute={attribute}
        component={resolveEditComponent(component) ?? component}
        error={validation.fieldErrors[attribute.code]}
        onChange={(value) => updateField(attribute.code, value)}
        value={fields[attribute.code] ?? ''}
      />
    </Box>
  );
  const editable = attributes.filter((attribute) => !attribute.readonly);

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
        ref={fieldsRef}
        sx={{ mt: 2, p: 2.5 }}
        variant="outlined"
      >
        <FieldDiagnosticsContext value={diagnostics}>
          <RevealFieldContext value={revealField}>
            <FieldRevealContext value={revealRequest}>
              {activeViewName === sandboxEditTab && validation.formError && (
                <Alert severity="error" sx={{ mb: 2 }}>
                  {validation.formError}
                </Alert>
              )}
              {activeViewName === sandboxEditTab ? (
                <EditableRecordLayout
                  attributes={attributes}
                  fallbackVisibilityScope="detail"
                  headingAttributes={headingEditableAttributes(
                    editable,
                    views.detail,
                  )}
                  headingLevel="h3"
                  otherAttributes={unplacedEditableAttributes(
                    editable,
                    views.detail,
                  )}
                  renderEditor={renderEditor}
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
            </FieldRevealContext>
          </RevealFieldContext>
        </FieldDiagnosticsContext>
      </Paper>
    </>
  );
};
