import {
  Autocomplete,
  Chip,
  createFilterOptions,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useForm, useStore } from '@tanstack/react-form';
import type { KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import type { Attribute } from '../records/api';
import { PRESENCE_FILTER_OPERATOR } from '../records/constants';
import { ValueTypeIcon } from '../records/components/ValueTypeLabel';
import { attributeLabel } from '../records/recordDisplay';
import { statusConfiguration, statusOptionLabel } from '../records/status';
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

// Long blueprints offer many fields, so match typed text against the label
// and the code, which also covers relationship paths such as family.name.
const filterFieldOptions = createFilterOptions<Attribute>({
  stringify: (attribute) => `${attributeLabel(attribute)} ${attribute.code}`,
});

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
      <Stack spacing={3}>
        <form.Field name="field">
          {(field) => (
            <Autocomplete
              autoHighlight
              filterOptions={filterFieldOptions}
              getOptionKey={(item) => item.code}
              getOptionLabel={attributeLabel}
              isOptionEqualToValue={(option, value) =>
                option.code === value.code
              }
              noOptionsText={t('explorer.noMatchingFilterFields')}
              onChange={(_, nextAttribute) => {
                if (
                  nextAttribute &&
                  isRelationshipFilterAttribute(nextAttribute)
                ) {
                  onSelectRelationship(nextAttribute);
                  return;
                }
                field.handleChange(nextAttribute?.code ?? '');
                form.setFieldValue(
                  'operator',
                  operatorsForAttribute(nextAttribute ?? undefined)[0] ??
                    defaultAttributeFilterOperator,
                );
                form.setFieldValue('value', '');
              }}
              openOnFocus
              options={attributes.filter(
                (item) => !editing || !isRelationshipFilterAttribute(item),
              )}
              renderInput={(params) => (
                <TextField {...params} label={t('explorer.filterField')} />
              )}
              renderOption={({ key, ...props }, item) => (
                <li data-value={item.code} key={key} {...props}>
                  <Stack
                    direction="row"
                    spacing={1}
                    sx={{ alignItems: 'center' }}
                  >
                    <ValueTypeIcon valueType={item.value_type} />
                    <Chip label={blueprintName} size="small" />
                    <Typography>{attributeLabel(item)}</Typography>
                  </Stack>
                </li>
              )}
              value={attribute ?? null}
            />
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
