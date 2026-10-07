import { useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Button, Typography } from '@mui/material';
import { useEffect, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { checkViolationError } from '../../../api/checkViolations';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { useViolationText } from '../../../components/useViolationText';
import { principalConfiguration } from '../../principals/principal';
import { EntityView } from '../../views/components/EntityView';
import type { ResolvedValue } from '../../views/components/ValueField';
import { entityHeadingComponentId } from '../../views/components/blocks/EntityHeadingDefinition';
import { resolveEditComponent } from '../../views/components/registry';
import {
  viewFieldComponents,
  viewFieldEditors,
} from '../../views/viewFieldComponents';
import {
  getEntityForm,
  type Attribute,
  type ComponentReference,
  type EntityFormResponse,
  type StatusTransitionAccess,
} from '../api';
import { violationFieldErrors } from '../checkViolations';
import { validateEntityForm, valuesForForm } from '../entityForm';
import {
  editableFormAttributes,
  unplacedEditableAttributes,
  type ResolvedFormValues,
} from '../entityFormAttributes';
import { schemaMismatchField } from '../entityFieldSaves';
import { entityQueryKeys } from '../queryKeys';
import { statusConfiguration, statusLocks } from '../status';
import { useEntityFieldSaves } from '../useEntityFieldSaves';
import { attributeValueTypes } from '../valueTypes';
import { EntityFormAttributeEditor } from './EntityFormAttributeEditor';
import { InlineFieldEditor } from './InlineFieldEditor';

type Props = {
  entityId: string;
  form: EntityFormResponse;
  attributes: readonly Attribute[];
  contextId: string | null;
  defaultContextId: string | null;
  statusParentContextIds: readonly string[];
  statusTransitions?: readonly StatusTransitionAccess[];
  resolvedValues: Record<string, ResolvedValue>;
  reusableResolvedValues: Record<string, ResolvedValue>;
  view: EntityFormResponse['blueprint']['blueprint']['views']['detail'];
  renderAttributeDecoration?: (attribute: Attribute) => ReactNode;
  renderAttributePanel?: (attribute: Attribute) => ReactNode;
  renderFilePanel?: (attribute: Attribute, fileId: string) => ReactNode;
  /** Reports whether changes are waiting to be saved in this context. */
  onPendingChange?: (pending: boolean) => void;
};

/** Changes of these kinds are final choices rather than typing. */
const commitsImmediately = (attribute: Attribute) =>
  attribute.value_type === attributeValueTypes.relationship ||
  attribute.value_type === attributeValueTypes.boolean ||
  attribute.value_type === attributeValueTypes.file ||
  statusConfiguration(attribute) !== undefined ||
  principalConfiguration(attribute) !== undefined;

/**
 * The entity's detail view with every field the user may change rendered as
 * an always-editable control. Each committed field saves on its own.
 */
export const EntityInlineFields = ({
  entityId,
  form,
  attributes,
  contextId,
  defaultContextId,
  statusParentContextIds,
  statusTransitions = [],
  resolvedValues,
  reusableResolvedValues,
  view,
  renderAttributeDecoration,
  renderAttributePanel,
  renderFilePanel,
  onPendingChange,
}: Props) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const violationText = useViolationText();
  const allAttributes = [...attributes, ...form.reusable_attributes];
  const existingValues = [...form.values, ...form.reusable_values];
  const editable = form.can_write
    ? editableFormAttributes(allAttributes, {
        contextId,
        defaultContextId,
        usesDefaultEditView: false,
      })
    : [];
  const locks = statusLocks(
    allAttributes,
    existingValues,
    contextId,
    statusParentContextIds,
  );
  const changeable = editable.filter(
    (attribute) => locks[attribute.code] === undefined,
  );
  const changeableCodes = changeable.map((attribute) => attribute.code);
  const editComponents = new Map(
    [...viewFieldComponents(view)].flatMap(([code, component]) => {
      const editor = resolveEditComponent(component);
      return editor ? [[code, editor] as const] : [];
    }),
  );
  const fieldRules = viewFieldEditors(editComponents, changeable);
  const saves = useEntityFieldSaves({
    entityId,
    contextId,
    attributes: changeable,
    savedFields: valuesForForm(allAttributes, existingValues, contextId),
    updatedAt: form.entity.updated_at,
    fieldRules,
  });
  const hasPending = Object.keys(saves.pending).length > 0;
  useEffect(() => onPendingChange?.(hasPending), [hasPending, onPendingChange]);

  const violations = checkViolationError(saves.error);
  const placedViolations = violations
    ? violationFieldErrors(
        violations.violations,
        changeableCodes,
        violationText,
      )
    : undefined;
  const schemaField = schemaMismatchField(saves.error, changeableCodes);
  const serverErrors: Record<string, string> = {
    ...placedViolations?.fieldErrors,
    ...(schemaField && {
      [schemaField.code]: schemaField.missing
        ? t('entities.valueRequired')
        : t('entities.schemaValidationFailed'),
    }),
  };
  const fieldErrors = Object.fromEntries(
    Object.keys(saves.pending).map((code) => [
      code,
      serverErrors[code] ?? t('entities.changeNotSaved'),
    ]),
  );
  // Errors no field can show, and violations of fields not on the page.
  const generalError =
    saves.error &&
    !saves.conflict &&
    (Object.keys(serverErrors).length === 0 ||
      (placedViolations?.unplaced.length ?? 0) > 0)
      ? saves.error
      : undefined;

  const resolveConflict = async (resolution: 'keepMine' | 'useTheirs') => {
    const latest = await client.fetchQuery({
      queryKey: entityQueryKeys.form(entityId),
      queryFn: ({ signal }) => getEntityForm(entityId, signal),
      staleTime: 0,
    });
    saves.resolveConflict(resolution, {
      savedFields: valuesForForm(
        [...latest.blueprint.attributes, ...latest.reusable_attributes],
        [...latest.values, ...latest.reusable_values],
        contextId,
      ),
      updatedAt: latest.entity.updated_at,
    });
  };

  const renderEditor = (
    attribute: Attribute,
    component?: ComponentReference | null,
  ) => {
    if (!changeableCodes.includes(attribute.code)) return null;
    const editComponent =
      resolveEditComponent(component) ?? editComponents.get(attribute.code);
    const rules = fieldRules.get(attribute.code);
    return (
      <InlineFieldEditor
        immediate={commitsImmediately(attribute)}
        onCommit={(value) => saves.commit(attribute.code, value)}
        onRevert={() => saves.revert(attribute.code)}
        validate={(value) =>
          validateEntityForm(
            [attribute],
            { [attribute.code]: value },
            [],
            undefined,
            undefined,
            rules ? new Map([[attribute.code, rules]]) : undefined,
          ).fieldErrors[attribute.code]
        }
        value={saves.fields[attribute.code] ?? ''}
      >
        {({ value, error, onChange }) => (
          <EntityFormAttributeEditor
            attribute={attribute}
            component={editComponent}
            contextId={contextId}
            defaultContextId={defaultContextId}
            entityId={entityId}
            existingValues={existingValues}
            fieldErrors={error ? { [attribute.code]: error } : fieldErrors}
            highlightedAttributes={[]}
            lockedAttributes={locks}
            migrationReviewMessages={{}}
            onChange={onChange}
            onEntityUpdated={saves.noteEntityUpdated}
            resolvedValues={resolvedValues as ResolvedFormValues}
            statusParentContextIds={statusParentContextIds}
            statusTransitions={statusTransitions}
            value={value}
          />
        )}
      </InlineFieldEditor>
    );
  };
  const viewProps = {
    contextId: contextId ?? undefined,
    entityId,
    renderAttributeDecoration,
    renderAttributePanel,
    renderFilePanel,
    renderEditor,
  };
  const unplaced = unplacedEditableAttributes(
    changeable.filter((attribute) =>
      attributes.some((placed) => placed.code === attribute.code),
    ),
    view,
    entityHeadingComponentId,
  );

  return (
    <>
      {saves.conflict && (
        <Alert
          action={
            <Box sx={{ display: 'flex', gap: 1 }}>
              <Button
                color="inherit"
                onClick={() => void resolveConflict('keepMine')}
                size="small"
              >
                {t('entities.keepMyChanges')}
              </Button>
              <Button
                color="inherit"
                onClick={() => void resolveConflict('useTheirs')}
                size="small"
              >
                {t('entities.useLatestValues')}
              </Button>
            </Box>
          }
          severity="warning"
          sx={{ mb: 2 }}
        >
          {t('entities.changedByAnotherEditor')}
        </Alert>
      )}
      {generalError && (
        <ApiErrorAlert
          error={generalError}
          violations={placedViolations?.unplaced}
          sx={{ mb: 2 }}
        />
      )}
      <Typography
        aria-live="polite"
        color="text.secondary"
        sx={{ display: 'block', minHeight: '1.5em', mb: 1 }}
        variant="caption"
      >
        {saves.saving
          ? t('entities.savingChanges')
          : hasPending
            ? t('entities.unsavedChangeCount', {
                count: Object.keys(saves.pending).length,
              })
            : null}
      </Typography>
      <EntityView
        {...viewProps}
        attributes={attributes}
        fallbackVisibilityScope="detail"
        skipComponentId={entityHeadingComponentId}
        values={resolvedValues}
        view={view}
      />
      {unplaced.length > 0 && (
        <Box component="section" sx={{ mt: 4 }}>
          <Typography component="h2" sx={{ mb: 2 }} variant="h6">
            {t('entities.otherAttributes')}
          </Typography>
          <EntityView
            {...viewProps}
            attributes={unplaced}
            values={resolvedValues}
          />
        </Box>
      )}
      {form.reusable_attributes.length > 0 && (
        <Box component="section" sx={{ mt: 4 }}>
          <Typography component="h2" sx={{ mb: 2 }} variant="h6">
            {t('entities.additionalAttributes')}
          </Typography>
          <EntityView
            {...viewProps}
            attributes={form.reusable_attributes}
            values={reusableResolvedValues}
          />
        </Box>
      )}
    </>
  );
};
