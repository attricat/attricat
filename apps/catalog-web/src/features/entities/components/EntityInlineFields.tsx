import { useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import { forwardRef, useImperativeHandle, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { checkViolationError } from '../../../api/checkViolations';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { useViolationText } from '../../../components/useViolationText';
import { useBeforeUnloadWarning } from '../../drafts/useBeforeUnloadWarning';
import { principalConfiguration } from '../../principals/principal';
import {
  EditableEntityLayout,
  EditableEntitySection,
} from '../../views/components/EditableEntityLayout';
import { EntityView } from '../../views/components/EntityView';
import type { ResolvedValue } from '../../views/components/ValueField';
import {
  resolveEditComponent,
  resolveViewComponent,
} from '../../views/components/registry';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../../views/viewFieldComponents';
import {
  type Attribute,
  type ComponentReference,
  type EntityFormResponse,
  type StatusTransitionAccess,
} from '../api';
import { violationFieldErrors } from '../checkViolations';
import {
  entityFormValidationMessages,
  validateEntityForm,
  valuesForForm,
} from '../entityForm';
import {
  editableFormAttributes,
  entitySchemaRequiredAttributes,
  headingEditableAttributes,
  smartFillFormFields,
  unplacedEditableAttributes,
  type ResolvedFormValues,
} from '../entityFormAttributes';
import { schemaMismatchField } from '../entityFieldSaves';
import { entityFormOptions } from '../queryOptions';
import { statusConfiguration, statusLocks } from '../status';
import {
  useEntityFieldSaves,
  type ConflictResolution,
} from '../useEntityFieldSaves';
import { attributeValueTypes } from '../valueTypes';
import { EntityFormAttributeEditor } from './EntityFormAttributeEditor';
import { InlineFieldEditor } from './InlineFieldEditor';
import { ReusableAttributeAttachControl } from './ReusableAttributeAttachControl';
import { UnsavedFieldChangesGuard } from './UnsavedFieldChangesGuard';

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
  /** Lays the view out in one column, as in a narrow panel. */
  singleColumn?: boolean;
};

export type EntityInlineFieldsHandle = {
  /** Current field values, including changes that are not saved yet. */
  getDraftValues: () => Record<string, string>;
  /** Saves Smart Fill suggestions for editable fields as one change. */
  applySmartFillValues: (values: Record<string, string>) => void;
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
export const EntityInlineFields = forwardRef<EntityInlineFieldsHandle, Props>(
  (
    {
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
      singleColumn,
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const client = useQueryClient();
    const violationText = useViolationText();
    const allAttributes = [...attributes, ...form.reusable_attributes];
    const existingValues = [...form.values, ...form.reusable_values];
    const editable = form.can_write
      ? editableFormAttributes(allAttributes, {
          contextId,
          defaultContextId,
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
    const editComponents = viewFieldEditComponents(view);
    const fieldRules = viewFieldEditors(editComponents, changeable);
    // The entity schema only applies to values in the default context.
    const requiredAttributes =
      contextId === defaultContextId
        ? entitySchemaRequiredAttributes(form.blueprint.blueprint.entity_schema)
        : [];
    const saves = useEntityFieldSaves({
      entityId,
      contextId,
      attributes: changeable,
      savedFields: valuesForForm(allAttributes, existingValues, contextId),
      updatedAt: form.entity.updated_at,
      fieldRules,
      onPendingChange,
    });
    const hasPending = Object.keys(saves.pending).length > 0;
    // Pending changes left after a failed or conflicting save; leaving the
    // entity drops them. Changes still being saved finish on their own.
    const unsaved = hasPending && !saves.saving;
    useImperativeHandle(ref, () => ({
      getDraftValues: () => ({ ...saves.fields }),
      applySmartFillValues: (values) => {
        const fields = smartFillFormFields(changeable, values);
        if (Object.keys(fields).length > 0) saves.commitMany(fields);
      },
    }));
    // Leaving would drop changes that are not saved yet.
    useBeforeUnloadWarning(hasPending || saves.saving);

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
    // A change only reads as not saved once a save failed; marking fields
    // while a save is in flight would shift the layout under the pointer.
    const fieldErrors = saves.error
      ? Object.fromEntries(
          Object.keys(saves.pending).map((code) => [
            code,
            serverErrors[code] ?? t('entities.changeNotSaved'),
          ]),
        )
      : {};
    // Errors no field can show, and violations of fields not on the page.
    const generalError =
      saves.error &&
      !saves.conflict &&
      (Object.keys(serverErrors).length === 0 ||
        (placedViolations?.unplaced.length ?? 0) > 0)
        ? saves.error
        : undefined;

    const resolveConflict = async (resolution: ConflictResolution) => {
      const latest = await client.fetchQuery({
        ...entityFormOptions(entityId),
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
      const display = resolveViewComponent(component);
      const KeptDisplay = display?.showsWhileEditing
        ? display.valueRenderer
        : undefined;
      const editor = (
        <InlineFieldEditor
          immediate={commitsImmediately(attribute)}
          onCommit={(value) => saves.commit(attribute.code, value)}
          onRevert={() => saves.revert(attribute.code)}
          validate={(value) =>
            validateEntityForm(
              [attribute],
              { [attribute.code]: value },
              requiredAttributes,
              undefined,
              {
                ...entityFormValidationMessages(),
                // Matches a required value the server rejects.
                required: t('entities.valueRequired'),
              },
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
      return KeptDisplay ? (
        <Stack spacing={1}>
          <KeptDisplay
            attribute={attribute}
            component={component}
            contextId={contextId ?? undefined}
            entityId={entityId}
            value={resolvedValues[attribute.code]?.value}
          />
          {editor}
        </Stack>
      ) : (
        editor
      );
    };
    const viewProps = {
      contextId: contextId ?? undefined,
      entityId,
      renderAttributeDecoration,
      renderAttributePanel,
      renderFilePanel,
      renderEditor,
      singleColumn,
    };
    const blueprintChangeable = changeable.filter((attribute) =>
      attributes.some((placed) => placed.code === attribute.code),
    );

    return (
      <>
        {unsaved && <UnsavedFieldChangesGuard />}
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
            action={
              unsaved && (
                <Button color="inherit" onClick={saves.retry} size="small">
                  {t('errors.retry')}
                </Button>
              )
            }
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
        <EditableEntityLayout
          {...viewProps}
          attributes={attributes}
          fallbackVisibilityScope="detail"
          headingAttributes={headingEditableAttributes(
            blueprintChangeable,
            view,
          )}
          otherAttributes={unplacedEditableAttributes(
            blueprintChangeable,
            view,
          )}
          values={resolvedValues}
          view={view}
        />
        {form.reusable_attributes.length > 0 && (
          <EditableEntitySection
            headingLevel="h2"
            title={t('entities.additionalAttributes')}
          >
            <EntityView
              {...viewProps}
              attributes={form.reusable_attributes}
              values={reusableResolvedValues}
            />
          </EditableEntitySection>
        )}
        {form.can_write && (
          <Box sx={{ mt: 3 }}>
            {/* Attaching changes the entity; wait for field saves first. */}
            <ReusableAttributeAttachControl
              disabled={hasPending}
              entityId={entityId}
            />
          </Box>
        )}
      </>
    );
  },
);
