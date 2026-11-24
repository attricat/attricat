import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, TableCell, TableRow } from '@mui/material';
import { CheckIcon, XIcon } from 'lucide-react';
import { useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';
import { LabeledIconButton } from '../../../components/LabeledIconButton';
import { compactIconSize } from '../../../components/iconSizes';
import { createRecord, type Attribute } from '../../records/api';
import { CompactFieldsContext } from '../../records/components/compactFields';
import { RecordFormAttributeEditor } from '../../records/components/RecordFormAttributeEditor';
import { RelationshipScopeContext } from '../../records/components/relationshipScope';
import { invalidateRecordSearches } from '../../records/invalidateRecord';
import { blueprintByCodeOptions } from '../../records/queryOptions';
import {
  formRecordSchema,
  recordFormValidationMessages,
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateRecordForm,
} from '../../records/recordForm';
import {
  editableFormAttributes,
  recordSchemaRequiredAttributes,
} from '../../records/recordFormAttributes';
import { attributeLabel } from '../../records/recordDisplay';
import { attributeValueKinds } from '../../records/valueTypes';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../viewFieldComponents';
import { cellEditorSx } from './cellEditorSx';

const noValues: Readonly<Record<string, string>> = {};

/**
 * A row for a new record linking to the page's record, typed into the
 * table's cells. Enter or the save button creates it with the link set, and
 * the row empties for the next one; Escape or discard closes it.
 */
export const NewIncomingRow = ({
  blueprintCode,
  columns,
  contextId,
  field,
  name,
  onClose,
  recordId,
}: {
  blueprintCode: string;
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
  const blueprint = useQuery(blueprintByCodeOptions(blueprintCode));
  const [fields, setFields] = useState(noValues);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string>();
  // Starts again in the first cell when the row opens and after each save.
  const [round, setRound] = useState(0);
  const attributes = blueprint.data?.attributes ?? [];
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
    blueprint.data?.blueprint.views.detail,
  );
  const fieldRules = viewFieldEditors(editComponents, editable);
  const requiredAttributes = recordSchemaRequiredAttributes(
    blueprint.data?.blueprint.record_schema,
  ).filter(
    (code) =>
      !attributes.find((attribute) => attribute.code === code)?.generated,
  );
  const values = { ...fields, [field]: recordId };
  const ready = Boolean(blueprint.data && link);

  useLayoutEffect(() => {
    if (!ready) return;
    row.current
      ?.querySelector<HTMLElement>('input:not([type="hidden"]), button')
      ?.focus();
  }, [ready, round]);

  const create = useMutation({
    meta: { toast: false },
    mutationFn: () => {
      if (!blueprint.data || !link) throw new Error('blueprint not loaded');
      const submitted = [...cells.values()];
      return createRecord({
        blueprint: {
          code: blueprint.data.blueprint.code,
          version: blueprint.data.blueprint.version,
        },
        values: [
          ...serializeAttributeValues(submitted, fields, contextId, fieldRules),
          ...relationshipTargetsForForm([...submitted, link], values).flatMap(
            (relationship) =>
              relationship.target_record_ids.map((target_record_id) => ({
                kind: attributeValueKinds.relationship,
                attribute_code: relationship.attribute_code,
                context_id: contextId,
                target_record_id,
              })),
          ),
        ],
      });
    },
    onSuccess: async () => {
      setFields(noValues);
      setRound((count) => count + 1);
      await invalidateRecordSearches(client);
    },
  });

  const save = () => {
    if (!blueprint.data || !link || create.isPending) return;
    const validation = validateRecordForm(
      [...cells.values(), link],
      values,
      requiredAttributes,
      formRecordSchema(blueprint.data.blueprint.record_schema, attributes),
      {
        ...recordFormValidationMessages(),
        required: t('records.valueRequired'),
        uneditableRequired: (code) =>
          t('records.uneditableRequiredValue', {
            attribute: attributeLabel(
              attributes.find((attribute) => attribute.code === code) ?? {
                code,
              },
            ),
          }),
      },
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
            <TableCell align="right" sx={{ whiteSpace: 'nowrap', width: '1%' }}>
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
      {(blueprint.error || create.error || formError) && (
        <TableRow>
          <TableCell colSpan={colSpan}>
            {formError ? (
              <Alert severity="error">{formError}</Alert>
            ) : (
              <ApiErrorAlert error={blueprint.error ?? create.error} />
            )}
          </TableCell>
        </TableRow>
      )}
    </>
  );
};
