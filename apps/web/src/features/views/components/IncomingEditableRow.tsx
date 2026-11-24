import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Box, Button, TableCell, TableRow } from '@mui/material';
import { RotateCcwIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';
import type { Attribute } from '../../records/api';
import type { ConflictResolution } from '../../records/constants';
import { CompactFieldsContext } from '../../records/components/compactFields';
import { FieldSaveStatus } from '../../records/components/FieldSaveStatus';
import { InlineFieldEditor } from '../../records/components/InlineFieldEditor';
import { RecordFormAttributeEditor } from '../../records/components/RecordFormAttributeEditor';
import { RelationshipScopeContext } from '../../records/components/relationshipScope';
import { SaveConflictAlert } from '../../records/components/SaveConflictAlert';
import {
  changeableInlineAttributes,
  commitsImmediately,
} from '../../records/inlineFields';
import { movementManagedCodes } from '../../records/movements';
import {
  entryValidationMessages,
  validateRecordForm,
  valuesForForm,
} from '../../records/recordForm';
import { recordSchemaRequiredAttributes } from '../../records/recordFormAttributes';
import {
  recordFormOptions,
  resolvedRecordPreviewOptions,
} from '../../records/queryOptions';
import { attributeLabel } from '../../records/recordDisplay';
import { useRecordFieldSaves } from '../../records/useRecordFieldSaves';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../viewFieldComponents';
import { AttributeValue } from './values/AttributeValue';
import { cellEditorSx, rowActionsCellSx } from './cellEditorSx';
import { NotSetValue } from './values/NotSetValue';

/** Lines are kept in the default context, which has no parent contexts. */
const noParentContexts: readonly string[] = [];

/**
 * One record linking to the page's record, as a table row whose cells edit
 * its values in place. Each committed cell saves on its own, as fields do on
 * the record's page; cells the user may not change show their value.
 */
export const IncomingEditableRow = ({
  actions,
  columns,
  contextId,
  editable,
  rowId,
}: {
  actions: ReactNode;
  columns: readonly Attribute[];
  contextId: string | null;
  /** Whether the page allows changing linked records at all. */
  editable: boolean;
  rowId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const preview = useQuery(resolvedRecordPreviewOptions(rowId, contextId));
  const form = useQuery({ ...recordFormOptions(rowId), enabled: editable });
  const attributes = form.data
    ? [...form.data.blueprint.attributes, ...form.data.reusable_attributes]
    : [];
  const existingValues = form.data
    ? [...form.data.values, ...form.data.reusable_values]
    : [];
  const { changeable, locks } =
    editable && form.data
      ? changeableInlineAttributes(form.data, attributes, {
          contextId,
          defaultContextId: contextId,
          statusParentContextIds: noParentContexts,
        })
      : { changeable: [], locks: {} };
  const changeableCodes = new Set(changeable.map((item) => item.code));
  const view = form.data?.blueprint.blueprint.views.detail;
  const editComponents = viewFieldEditComponents(view);
  const fieldRules = viewFieldEditors(editComponents, changeable);
  const requiredAttributes = recordSchemaRequiredAttributes(
    form.data?.blueprint.blueprint.record_schema,
  );
  const movementManaged = movementManagedCodes(
    form.data?.blueprint.blueprint.movements,
  );
  const latestFields = valuesForForm(attributes, existingValues, contextId);
  const saves = useRecordFieldSaves({
    recordId: rowId,
    contextId,
    attributes: changeable,
    savedFields: latestFields,
    updatedAt: form.data?.record.updated_at,
    fieldRules,
  });
  const resolvedValues = preview.data?.values ?? {};
  // A failed save marks the fields it would have changed.
  const fieldErrors: Record<string, string> = saves.error
    ? Object.fromEntries(
        Object.keys(saves.pending).map((code) => [
          code,
          t('records.changeNotSaved'),
        ]),
      )
    : {};
  const conflictChoice = useMutation({
    meta: { toast: false },
    mutationFn: async (resolution: ConflictResolution) => {
      const latest = await client.fetchQuery({
        ...recordFormOptions(rowId),
        staleTime: 0,
      });
      saves.resolveConflict(resolution, {
        savedFields: valuesForForm(
          [...latest.blueprint.attributes, ...latest.reusable_attributes],
          [...latest.values, ...latest.reusable_values],
          contextId,
        ),
        updatedAt: latest.record.updated_at,
      });
    },
  });
  const colSpan = columns.length + 1;

  const renderCell = (column: Attribute) => {
    // The record's own blueprint version decides how the value is edited.
    const attribute = attributes.find((item) => item.code === column.code);
    if (!attribute || !changeableCodes.has(attribute.code)) {
      const value = resolvedValues[column.code]?.value;
      return value === undefined || value === null ? (
        <NotSetValue />
      ) : (
        <AttributeValue attribute={column} recordId={rowId} value={value} />
      );
    }
    const editComponent = editComponents.get(attribute.code);
    const rules = fieldRules.get(attribute.code);
    const required = requiredAttributes.includes(attribute.code);
    return (
      <Box data-field-code={attribute.code} sx={cellEditorSx}>
        <InlineFieldEditor
          immediate={commitsImmediately(attribute, editComponent)}
          onCommit={(value) => saves.commit(attribute.code, value)}
          onRevert={() => saves.revert(attribute.code)}
          validate={(value) =>
            validateRecordForm(
              [attribute],
              { [attribute.code]: value },
              requiredAttributes,
              undefined,
              entryValidationMessages(),
              rules ? new Map([[attribute.code, rules]]) : undefined,
            ).fieldErrors[attribute.code]
          }
          value={saves.fields[attribute.code] ?? ''}
        >
          {({ value, error, onChange }) => (
            <RecordFormAttributeEditor
              attribute={attribute}
              component={editComponent}
              contextId={contextId}
              defaultContextId={contextId}
              existingValues={existingValues}
              fieldErrors={error ? { [attribute.code]: error } : fieldErrors}
              lockedAttributes={locks}
              movementManaged={movementManaged}
              onChange={onChange}
              onRecordUpdated={saves.noteRecordUpdated}
              recordId={rowId}
              required={required}
              resolvedValues={resolvedValues}
              value={value}
            />
          )}
        </InlineFieldEditor>
        <FieldSaveStatus
          onHold={saves.holdLastSave}
          onUndo={saves.undoLastSave}
          state={saves.fieldState(attribute.code)}
          undoFields={saves.lastSaveFields.map((code) =>
            attributeLabel(
              attributes.find((item) => item.code === code) ?? column,
            ),
          )}
        />
      </Box>
    );
  };

  return (
    <>
      <RelationshipScopeContext value={saves.fields}>
        <CompactFieldsContext value>
          <TableRow hover>
            {columns.map((column) => (
              <TableCell key={column.code}>{renderCell(column)}</TableCell>
            ))}
            <TableCell align="right" sx={rowActionsCellSx}>
              {actions}
            </TableCell>
          </TableRow>
        </CompactFieldsContext>
      </RelationshipScopeContext>
      {saves.conflict && (
        <TableRow>
          <TableCell colSpan={colSpan}>
            <SaveConflictAlert
              attributes={changeable.filter(
                (attribute) => attribute.code in saves.pending,
              )}
              disabled={conflictChoice.isPending}
              latest={latestFields}
              onResolve={(resolution) => conflictChoice.mutate(resolution)}
              pending={saves.pending}
            />
            <ApiErrorAlert error={conflictChoice.error} />
          </TableCell>
        </TableRow>
      )}
      {saves.error && !saves.conflict && (
        <TableRow>
          <TableCell colSpan={colSpan}>
            <ApiErrorAlert
              action={
                !saves.saving && (
                  <Button
                    color="inherit"
                    onClick={saves.retry}
                    size="small"
                    startIcon={<RotateCcwIcon />}
                  >
                    {t('errors.retry')}
                  </Button>
                )
              }
              error={saves.error}
            />
          </TableCell>
        </TableRow>
      )}
    </>
  );
};
