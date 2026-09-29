import { Chip, MenuItem, Stack, TextField, Typography } from '@mui/material';
import { useForm, useStore } from '@tanstack/react-form';
import type { KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import type { Attribute } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import { operatorsForValueType } from './attributeFilters';
import {
  attributeFilterInputType,
  booleanFilterValues,
  type AttributeFilterDraft,
  isAttributeFilterValueValid,
  parseAttributeFilterValue,
} from './attributeFilterValues';
import { defaultAttributeFilterOperator } from './constants';
import {
  isRelationshipFilterAttribute,
  type RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import type { AttributeFilter } from './search';
import { useTimeZone } from '../../time/useInstantFormat';

const describeDraft = (
  draft: AttributeFilterDraft,
  attributes: Attribute[],
) => {
  const attribute = attributes.find((item) => item.code === draft.field);
  const availableOperators = operatorsForValueType(
    attribute?.value_type ?? 'string',
  );
  return {
    attribute,
    availableOperators,
    effectiveOperator: availableOperators.includes(draft.operator)
      ? draft.operator
      : defaultAttributeFilterOperator,
    relationship: attribute && isRelationshipFilterAttribute(attribute),
    valueIsValid: isAttributeFilterValueValid(
      attribute?.value_type,
      draft.value,
    ),
  };
};

type Props = {
  attributes: Attribute[];
  blueprintName: string;
  editing: boolean;
  initialDraft: AttributeFilterDraft;
  maximumReached: boolean;
  onClose: () => void;
  onSelectRelationship: (attribute: RelationshipFilterAttribute) => void;
  onSubmit: (filter: AttributeFilter) => void;
  open: boolean;
  relationshipPathsLoading: boolean;
};

export const AttributeFilterDialog = ({
  attributes,
  blueprintName,
  editing,
  initialDraft,
  maximumReached,
  onClose,
  onSelectRelationship,
  onSubmit,
  open,
  relationshipPathsLoading,
}: Props) => {
  const { t } = useTranslation();
  const timeZone = useTimeZone();
  const addBlocked = maximumReached && !editing;
  const form = useForm({
    defaultValues: initialDraft,
    onSubmit: ({ value }) => {
      const draft = describeDraft(value, attributes);
      if (!draft.attribute) return;
      if (isRelationshipFilterAttribute(draft.attribute)) {
        onSelectRelationship(draft.attribute);
        return;
      }
      if (!draft.valueIsValid || addBlocked) return;
      onSubmit({
        field: draft.attribute.code,
        operator: draft.effectiveOperator,
        value: parseAttributeFilterValue(
          draft.attribute.value_type,
          value.value,
          timeZone,
        ),
      });
    },
  });
  const values = useStore(form.store, (state) => state.values);
  const {
    attribute,
    availableOperators,
    effectiveOperator,
    relationship,
    valueIsValid,
  } = describeDraft(values, attributes);
  const submit = () => void form.handleSubmit();
  const submitOnEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    submit();
  };

  return (
    <RelationshipSelectorDialog
      actions={{
        applyDisabled:
          !attribute || (!relationship && (!valueIsValid || addBlocked)),
        applyLabel: t(
          editing
            ? 'explorer.updateAttributeFilter'
            : 'explorer.applyAttributeFilter',
        ),
        cancelLabel: t('explorer.cancelAttributeFilter'),
        clearLabel: t('explorer.clearAttributeFilter'),
        onApply: submit,
        onClear: () => form.setFieldValue('value', ''),
      }}
      closeLabel={t('explorer.closeAttributeFilter')}
      onClose={onClose}
      open={open}
      selectedLabel={t('explorer.attributeFilterDescription')}
      title={t(
        editing
          ? 'explorer.editAttributeFilterTitle'
          : 'explorer.addAttributeFilterTitle',
      )}
    >
      <Stack spacing={1.5}>
        <form.Field name="field">
          {(field) => (
            <TextField
              fullWidth
              label={t('explorer.filterField')}
              onChange={(event) => {
                const nextField = event.target.value;
                const nextAttribute = attributes.find(
                  (item) => item.code === nextField,
                );
                if (
                  nextAttribute &&
                  isRelationshipFilterAttribute(nextAttribute)
                ) {
                  onSelectRelationship(nextAttribute);
                  return;
                }
                field.handleChange(nextField);
                form.setFieldValue('operator', defaultAttributeFilterOperator);
                form.setFieldValue('value', '');
              }}
              select
              value={field.state.value}
            >
              {attributes
                .filter(
                  (item) => !editing || !isRelationshipFilterAttribute(item),
                )
                .map((item) => (
                  <MenuItem key={item.code} value={item.code}>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: 'center' }}
                    >
                      <Chip label={blueprintName} size="small" />
                      {item.value_type === 'relationship' && (
                        <Chip label={t('explorer.relationship')} size="small" />
                      )}
                      <Typography>{attributeLabel(item)}</Typography>
                    </Stack>
                  </MenuItem>
                ))}
            </TextField>
          )}
        </form.Field>
        {relationshipPathsLoading && (
          <Typography color="text.secondary" variant="caption">
            {t('explorer.loadingRelationshipFields')}
          </Typography>
        )}
        {attribute && !relationship && (
          <>
            <form.Field name="operator">
              {(field) => (
                <TextField
                  fullWidth
                  label={t('explorer.operator')}
                  onChange={(event) => {
                    const validOperator = availableOperators.find(
                      (item) => item === event.target.value,
                    );
                    if (validOperator) field.handleChange(validOperator);
                  }}
                  select
                  value={effectiveOperator}
                >
                  {availableOperators.map((item) => (
                    <MenuItem key={item} value={item}>
                      {t(`explorer.filterOperators.${item}`)}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </form.Field>
            <form.Field name="value">
              {(field) =>
                attribute.value_type === 'boolean' ? (
                  <TextField
                    fullWidth
                    label={t('explorer.value')}
                    onChange={(event) => field.handleChange(event.target.value)}
                    onKeyDown={submitOnEnter}
                    select
                    value={field.state.value}
                  >
                    <MenuItem value={booleanFilterValues.true}>
                      {t('explorer.true')}
                    </MenuItem>
                    <MenuItem value={booleanFilterValues.false}>
                      {t('explorer.false')}
                    </MenuItem>
                  </TextField>
                ) : (
                  <TextField
                    fullWidth
                    slotProps={{
                      htmlInput:
                        attribute.value_type === 'integer'
                          ? { step: 1 }
                          : undefined,
                    }}
                    label={t('explorer.value')}
                    onChange={(event) => field.handleChange(event.target.value)}
                    onKeyDown={submitOnEnter}
                    type={attributeFilterInputType(attribute.value_type)}
                    value={field.state.value}
                  />
                )
              }
            </form.Field>
          </>
        )}
      </Stack>
    </RelationshipSelectorDialog>
  );
};
