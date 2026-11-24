import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, TableCell, TableRow } from '@mui/material';
import { CheckIcon, XIcon } from 'lucide-react';
import { useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';
import { LabeledIconButton } from '../../../components/LabeledIconButton';
import { compactIconSize } from '../../../components/iconSizes';
import {
  createRecord,
  type Attribute,
  type BlueprintWithAttributes,
} from '../../records/api';
import { CompactFieldsContext } from '../../records/components/compactFields';
import { RecordFormAttributeEditor } from '../../records/components/RecordFormAttributeEditor';
import { RelationshipScopeContext } from '../../records/components/relationshipScope';
import { invalidateRecordSearches } from '../../records/invalidateRecord';
import {
  formRecordSchema,
  entryValidationMessages,
  relationshipTargetsForForm,
  relationshipValueInputs,
  serializeAttributeValues,
  validateRecordForm,
} from '../../records/recordForm';
import {
  editableFormAttributes,
  recordSchemaRequiredAttributes,
} from '../../records/recordFormAttributes';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../viewFieldComponents';
import { cellEditorSx, rowActionsCellSx } from './cellEditorSx';

const noValues: Readonly<Record<string, string>> = {};

/**
 * A row for a new record linking to the page's record, typed into the
 * table's cells. Enter or the save button creates it with the link set, and
 * the row empties for the next one; Escape or discard closes it.
 */
export const NewIncomingRow = ({
  blueprint,
  columns,
  contextId,
  field,
  name,
  onClose,
  recordId,
}: {
  /** The linking records' blueprint, as the table's rows were read with. */
  blueprint: BlueprintWithAttributes;
  columns: readonly Attribute[];
  contextId: string | null;
  /** The relationship linking the new record to the page's record. */
  field: string;
  name: string;
  onClose: () => void;
  recordId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const row = useRef<HTMLTableRowElement>(null);
  const [fields, setFields] = useState(noValues);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string>();
  // Starts again in the first cell when the row opens and after each save.
  const [round, setRound] = useState(0);
  const attributes = blueprint.attributes;
  const editable = editableFormAttributes(attributes, {
    contextId,
    defaultContextId: contextId,
  }).filter((attribute) => attribute.code !== field);
  const cells = new Map(
    editable
      .filter((attribute) =>
        columns.some((column) => column.code === attribute.code),
      )
      .map((attribute) => [attribute.code, attribute]),
  );
  const link = attributes.find((attribute) => attribute.code === field);
  const editComponents = viewFieldEditComponents(
    blueprint.blueprint.views.detail,
  );
  const fieldRules = viewFieldEditors(editComponents, editable);
  const requiredAttributes = recordSchemaRequiredAttributes(
    blueprint.blueprint.record_schema,
  ).filter(
    (code) =>
      !attributes.find((attribute) => attribute.code === code)?.generated,
  );
  const values = { ...fields, [field]: recordId };

  useLayoutEffect(() => {
    row.current
      ?.querySelector<HTMLElement>('input:not([type="hidden"]), button')
      ?.focus();
  }, [round]);

  const create = useMutation({
    meta: { toast: false },
    mutationFn: () => {
      if (!link) throw new Error('the linking relationship is missing');
      const submitted = [...cells.values()];
      return createRecord({
        blueprint: {
          code: blueprint.blueprint.code,
          version: blueprint.blueprint.version,
        },
        values: [
          ...serializeAttributeValues(submitted, fields, contextId, fieldRules),
          ...relationshipValueInputs(
            relationshipTargetsForForm([...submitted, link], values),
            contextId,
          ),
        ],
      });
    },
    onSuccess: () => {
      setFields(noValues);
      setRound((count) => count + 1);
      // The row is free for the next record while the searches refetch.
      void invalidateRecordSearches(client);
    },
  });

  const save = () => {
    if (!link || create.isPending) return;
    const validation = validateRecordForm(
      [...cells.values(), link],
      values,
      requiredAttributes,
      formRecordSchema(blueprint.blueprint.record_schema, attributes),
      entryValidationMessages(attributes),
      fieldRules,
    );
    setFieldErrors(validation.fieldErrors);
    setFormError(validation.formError);
    if (Object.keys(validation.fieldErrors).length > 0 || validation.formError)
      return;
    create.mutate();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTableRowElement>) => {
    // Escape inside an open list or dialog closes that, not the row.
    if (event.defaultPrevented) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    } else if (
      event.key === 'Enter' &&
      event.target instanceof HTMLInputElement &&
      event.target.getAttribute('aria-expanded') !== 'true'
    ) {
      event.preventDefault();
      save();
    }
  };

  const colSpan = columns.length + 1;
  return (
    <>
      <RelationshipScopeContext value={values}>
        <CompactFieldsContext value>
          <TableRow
            aria-label={t('views.incomingTable.newRow', { name })}
            onKeyDown={onKeyDown}
            ref={row}
            selected
          >
            {columns.map((column) => {
              const attribute = cells.get(column.code);
              return (
                <TableCell key={column.code}>
                  {attribute && (
                    <Box sx={cellEditorSx}>
                      <RecordFormAttributeEditor
                        attribute={attribute}
                        component={editComponents.get(attribute.code)}
                        contextId={contextId}
                        defaultContextId={contextId}
                        disabled={create.isPending}
                        existingValues={[]}
                        fieldErrors={fieldErrors}
                        onChange={(value) => {
                          setFields((current) => ({
                            ...current,
                            [attribute.code]: value,
                          }));
                          setFieldErrors((current) => {
                            const next = { ...current };
                            delete next[attribute.code];
                            return next;
                          });
                        }}
                        required={requiredAttributes.includes(attribute.code)}
                        resolvedValues={{}}
                        value={fields[attribute.code] ?? ''}
                      />
                    </Box>
                  )}
                </TableCell>
              );
            })}
            <TableCell align="right" sx={rowActionsCellSx}>
              <LabeledIconButton
                label={t('views.incomingTable.saveNew', { name })}
                onClick={save}
                size="small"
              >
                <CheckIcon size={compactIconSize} />
              </LabeledIconButton>
              <LabeledIconButton
                label={t('views.incomingTable.discardNew', { name })}
                onClick={onClose}
                size="small"
              >
                <XIcon size={compactIconSize} />
              </LabeledIconButton>
            </TableCell>
          </TableRow>
        </CompactFieldsContext>
      </RelationshipScopeContext>
      {(create.error || formError) && (
        <TableRow>
          <TableCell colSpan={colSpan}>
            {formError ? (
              <Alert severity="error">{formError}</Alert>
            ) : (
              <ApiErrorAlert error={create.error} />
            )}
          </TableCell>
        </TableRow>
      )}
    </>
  );
};
