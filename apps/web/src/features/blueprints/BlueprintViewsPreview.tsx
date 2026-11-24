import {
  Alert,
  Box,
  IconButton,
  Paper,
  Stack,
  Tab,
  Tabs,
  Tooltip,
  Typography,
} from '@mui/material';
import { CodeXmlIcon } from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { scanYaml, type YamlOutline } from '../../components/yamlOutline';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import {
  attributeTarget,
  definitionSectionAt,
  definitionSectionKinds,
  viewDefinition,
} from '../definition-editor/definitionNavigation';
import {
  useDefinitionCursor,
  useRevealDefinition,
} from '../definition-editor/definitionReveal';
import type {
  Attribute,
  Blueprint,
  ComponentReference,
  JsonSchema,
  ViewDefinition,
} from '../records/api';
import { FieldDiagnosticsContext } from '../records/fieldDiagnostics';
import { resolvedRecordPreviewOptions } from '../records/queryOptions';
import { attributeLabel } from '../records/recordDisplay';
import { useRecordContextSelection } from '../records/useRecordContexts';
import { useBlueprintFieldDiagnostics } from '../records/useBlueprintFieldDiagnostics';
import { EditableRecordLayout } from '../views/components/EditableRecordLayout';
import { RecordView } from '../views/components/RecordView';
import { RecordWritesContext } from '../views/recordWrites';
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
import { sandboxEditTab, sandboxEditView } from './constants';
import { PreviewRecordPicker } from './PreviewRecordPicker';
import {
  isSandboxEditable,
  mergePreviewValues,
  previewRecordValues,
} from './previewRecordValues';
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
  blueprintCode,
  definition,
  recordSchema,
  revision,
  views,
}: {
  attributes: readonly Attribute[];
  /** Offers the blueprint's existing records to preview the views with. */
  blueprintCode?: string;
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
  // A chosen record fills the sandbox and lends its relationships and files.
  const [recordId, setRecordId] = useState<string>();
  const { contextId, contexts, setSelectedContext } =
    useRecordContextSelection();
  const record = useQuery(
    resolvedRecordPreviewOptions(
      recordId ?? '',
      recordId ? (contextId ?? null) : null,
    ),
  );
  const recordValues = useMemo(
    () =>
      record.data && recordId
        ? previewRecordValues(attributes, record.data.values)
        : undefined,
    [attributes, record.data, recordId],
  );
  // Fills the sandbox once for each record and context it is shown in.
  const recordKey = recordValues && `${recordId}:${contextId}`;
  const [filledFrom, setFilledFrom] = useState<string>();
  if (recordKey !== filledFrom) {
    setFilledFrom(recordKey);
    if (recordValues) setFields(recordValues.fields);
  }
  const chooseRecord = (id: string | undefined) => {
    setRecordId(id);
    if (!id) {
      setFields({});
      setFilledFrom(undefined);
    }
  };
  const sandboxValues = sandboxValuesForFields(attributes, fields);
  const values = recordValues
    ? mergePreviewValues(attributes, sandboxValues, recordValues.linked)
    : sandboxValues;
  const previewRecordId = recordValues ? recordId : undefined;
  const validation = sandboxValidation(attributes, fields, recordSchema);
  const diagnostics = useBlueprintFieldDiagnostics({
    attributes,
    recordSchema,
    definition,
    revision,
  });
  const revealDefinition = useRevealDefinition();
  const fieldDiagnostics = useMemo(() => {
    if (!revealDefinition || definition === undefined) return diagnostics;
    // Every field asks on each render, so the definition is scanned once.
    let outline: YamlOutline | undefined;
    return {
      ...diagnostics,
      showInDefinition: (code: string) => {
        outline ??= scanYaml(definition);
        const target = attributeTarget(outline, code);
        return target && (() => revealDefinition(target));
      },
    };
  }, [definition, diagnostics, revealDefinition]);
  // The sandbox editor lays fields out like the detail view.
  const viewName =
    activeViewName === sandboxEditTab ? sandboxEditView : activeViewName;
  const activeViewTarget =
    revealDefinition && definition !== undefined
      ? viewDefinition(definition, viewName)
      : undefined;
  const fieldsRef = useRef<HTMLDivElement>(null);
  const { request: revealRequest, reveal: revealField } =
    useFieldReveal(fieldsRef);

  // Follows the editor's cursor: an attribute shows in the sandbox editor,
  // a view in its tab, and the field either names is scrolled to.
  const cursor = useDefinitionCursor();
  const section = useMemo(
    () =>
      cursor === undefined || definition === undefined
        ? undefined
        : definitionSectionAt(definition, cursor),
    [cursor, definition],
  );
  const followedCode =
    section?.kind === definitionSectionKinds.attribute
      ? section.code
      : section?.field;
  const sectionKey = section && JSON.stringify(section);
  const [followedSection, setFollowedSection] = useState<string>();
  if (sectionKey !== followedSection) {
    setFollowedSection(sectionKey);
    if (section?.kind === definitionSectionKinds.attribute)
      setSelectedView(sandboxEditTab);
    else if (
      section &&
      section.name !== viewName &&
      tabs.some(([name]) => name === section.name)
    )
      setSelectedView(section.name);
    if (followedCode) revealField(followedCode, { focus: false });
  }
  const updateField = (code: string, value: string) =>
    setFields((current) => ({ ...current, [code]: value }));
  // Display components on the detail layout edit with their paired editor.
  const renderEditor = (
    attribute: Attribute,
    component?: ComponentReference | null,
  ) => (
    // Marks where the field is, to lead to it from a connected field.
    <Box data-field-code={attribute.code}>
      {previewRecordId && !isSandboxEditable(attribute) ? (
        // The sandbox cannot edit links; it shows the chosen record's.
        <RecordView
          attributes={[attribute]}
          contextId={contextId ?? undefined}
          recordId={previewRecordId}
          values={values}
        />
      ) : (
        <SandboxAttributeEditor
          attribute={attribute}
          component={resolveEditComponent(component) ?? component}
          error={validation.fieldErrors[attribute.code]}
          onChange={(value) => updateField(attribute.code, value)}
          value={fields[attribute.code] ?? ''}
        />
      )}
    </Box>
  );
  const editable = attributes.filter((attribute) => !attribute.readonly);

  return (
    <>
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        {t('blueprints.sandboxDescription')}
      </Typography>
      {blueprintCode && contextId && (
        <PreviewRecordPicker
          blueprintCode={blueprintCode}
          contextId={contextId}
          contexts={contexts.data ?? []}
          onContextChange={setSelectedContext}
          onRecordChange={chooseRecord}
          recordId={recordId}
        />
      )}
      {record.error && (
        <Alert severity="error" sx={{ mt: 1 }}>
          {t('blueprints.previewRecord.unavailable')}
        </Alert>
      )}
      {recordValues && recordValues.unfit.length > 0 && (
        <Alert severity="info" sx={{ mt: 1 }}>
          {t('blueprints.previewRecord.unfit', {
            attributes: recordValues.unfit
              .map((code) => {
                const attribute = attributes.find(
                  (candidate) => candidate.code === code,
                );
                return attribute ? attributeLabel(attribute) : code;
              })
              .join(', '),
          })}
        </Alert>
      )}
      <Stack direction="row" sx={{ alignItems: 'center', mt: 1 }}>
        <Tabs
          allowScrollButtonsMobile
          onChange={(_, value: string) => setSelectedView(value)}
          scrollButtons="auto"
          sx={{ flexGrow: 1, minWidth: 0 }}
          value={activeViewName}
          variant="scrollable"
        >
          {tabs.map(([name], index) => (
            <Tab
              {...tabId.tab(index)}
              key={name}
              label={
                name === sandboxEditTab ? t('blueprints.sandboxEdit') : name
              }
              value={name}
            />
          ))}
        </Tabs>
        {activeViewTarget && (
          <Tooltip
            title={t('definitionEditor.preview.showView', { view: viewName })}
          >
            <IconButton
              aria-label={t('definitionEditor.preview.showView', {
                view: viewName,
              })}
              onClick={() => revealDefinition?.(activeViewTarget)}
              size="small"
            >
              <CodeXmlIcon size={compactIconSize} />
            </IconButton>
          </Tooltip>
        )}
      </Stack>
      <Paper
        {...tabId.panel(activeViewIndex)}
        ref={fieldsRef}
        sx={{
          mt: 2,
          p: 2.5,
          ...(followedCode && {
            [`& [data-field-code=${JSON.stringify(followedCode)}]`]: {
              bgcolor: 'action.selected',
              borderRadius: 1,
              // A tinted margin inside a primary ring, so the field stands out.
              boxShadow: ({ palette, spacing }) =>
                `0 0 0 ${spacing(1)} ${palette.action.selected}, 0 0 0 calc(${spacing(1)} + 2px) ${palette.primary.main}`,
            },
          }),
        }}
        variant="outlined"
      >
        {/* The preview never changes a chosen record, such as its lines. */}
        <RecordWritesContext value={false}>
          <FieldDiagnosticsContext value={fieldDiagnostics}>
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
                    contextId={contextId ?? undefined}
                    recordId={previewRecordId}
                    renderEditor={renderEditor}
                    values={values}
                    view={activeView}
                  />
                ) : (
                  activeView && (
                    <RenderedBlueprintView
                      attributes={attributes}
                      contextId={contextId ?? undefined}
                      recordId={previewRecordId}
                      values={values}
                      view={activeView}
                    />
                  )
                )}
              </FieldRevealContext>
            </RevealFieldContext>
          </FieldDiagnosticsContext>
        </RecordWritesContext>
      </Paper>
    </>
  );
};
