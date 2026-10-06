import { Chip, MenuItem, Stack, TextField, Typography } from '@mui/material';
import { useForm, useStore } from '@tanstack/react-form';
import type { KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import type { Attribute } from '../entities/api';
import { PRESENCE_FILTER_OPERATOR } from '../entities/constants';
import { attributeLabel } from '../entities/entityDisplay';
import { statusConfiguration, statusOptionLabel } from '../entities/status';
import { CURRENT_USER_FILTER_VALUE } from '../principals/constants';
import {
  directoryOptions,
  principalConfiguration,
} from '../principals/principal';
import { usePrincipalDirectory } from '../principals/usePrincipalDirectory';
import { operatorsForAttribute } from './attributeFilters';
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
  const availableOperators = operatorsForAttribute(attribute);
  const effectiveOperator = availableOperators.includes(draft.operator)
    ? draft.operator
    : (availableOperators[0] ?? defaultAttributeFilterOperator);
  return {
    attribute,
    availableOperators,
    effectiveOperator,
    relationship: attribute && isRelationshipFilterAttribute(attribute),
    valueIsValid: isAttributeFilterValueValid(
      attribute?.value_type,
      draft.value,
      effectiveOperator,
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
          draft.effectiveOperator,
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
  const status = attribute && statusConfiguration(attribute);
  const principal = attribute && principalConfiguration(attribute);
  const presence = effectiveOperator === PRESENCE_FILTER_OPERATOR;
  const directory = usePrincipalDirectory(Boolean(principal) && !presence);
  // Filters may target former members and deleted teams, unlike assignment.
  // Attributes with a closed set of values are filtered by choosing one.
  const valueChoices: { value: string; label: string }[] | undefined = presence
    ? [
        { value: booleanFilterValues.true, label: t('explorer.valueIsSet') },
        {
          value: booleanFilterValues.false,
          label: t('explorer.valueIsNotSet'),
        },
      ]
    : attribute?.value_type === 'boolean'
      ? [
          { value: booleanFilterValues.true, label: t('explorer.true') },
          { value: booleanFilterValues.false, label: t('explorer.false') },
        ]
      : principal
        ? [
            {
              value: CURRENT_USER_FILTER_VALUE,
              label: t('explorer.assignedToMe'),
            },
            ...directoryOptions(directory.data, principal).map((option) => ({
              value: option.value,
              label: option.principal.label,
            })),
          ]
        : status
          ? status.options.map((option) => ({
              value: option.code,
              label: statusOptionLabel(option),
            }))
          : undefined;
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
                form.setFieldValue(
                  'operator',
                  operatorsForAttribute(nextAttribute)[0] ??
                    defaultAttributeFilterOperator,
                );
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
                    if (validOperator) {
                      if (
                        (validOperator === PRESENCE_FILTER_OPERATOR) !==
                        presence
                      ) {
                        form.setFieldValue('value', '');
                      }
                      field.handleChange(validOperator);
                    }
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
                valueChoices ? (
                  <TextField
                    fullWidth
                    label={t('explorer.value')}
                    onChange={(event) => field.handleChange(event.target.value)}
                    onKeyDown={submitOnEnter}
                    select
                    value={field.state.value}
                  >
                    {valueChoices.map((choice) => (
                      <MenuItem key={choice.value} value={choice.value}>
                        {choice.label}
                      </MenuItem>
                    ))}
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
