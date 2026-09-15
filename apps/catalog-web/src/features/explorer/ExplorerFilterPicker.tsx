import AddIcon from '@mui/icons-material/Add';
import {
  Button,
  Chip,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState, type KeyboardEvent } from 'react';
import { useQueries } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import { getBlueprintByCode, type Attribute } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import { attributeLabel } from '../entities/entity-display';
import {
  isFilterableAttribute,
  operatorsForValueType,
  type AttributeFilterOperator,
} from './attribute-filters';
import {
  isRelationshipFilterAttribute,
  type RelationshipFilterAttribute,
} from './relationship-filter-types';
import { maximumAttributeFilters, type AttributeFilter } from './search';

type Props = {
  filters: AttributeFilter[];
  attributes: Attribute[];
  blueprintName: string;
  pathAttributes?: { code: string; value_type: Attribute['value_type'] }[];
  relationshipAttributes?: RelationshipFilterAttribute[];
  onAdd: (filter: AttributeFilter) => void;
  onAddRelationship: (attribute: RelationshipFilterAttribute) => void;
  onRemove: (index: number) => void;
  onUpdate: (index: number, filter: AttributeFilter) => void;
};

export const ExplorerFilterPicker = ({
  filters,
  attributes,
  blueprintName,
  pathAttributes = [],
  relationshipAttributes = [],
  onAdd,
  onAddRelationship,
  onRemove,
  onUpdate,
}: Props) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [field, setField] = useState('');
  const [operator, setOperator] = useState<AttributeFilterOperator>('eq');
  const [value, setValue] = useState('');
  const discoverRelationshipPaths = open && editingIndex === null;
  const directRelationships = relationshipAttributes;
  const firstTargets = useQueries({
    queries: directRelationships.map((attribute) => ({
      queryKey: entityQueryKeys.blueprintByCode(
        attribute.target_blueprint_code,
        undefined,
      ),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getBlueprintByCode(attribute.target_blueprint_code, undefined, signal),
      enabled: discoverRelationshipPaths,
    })),
  });
  const secondRelationships = firstTargets.flatMap((result, index) =>
    (result.data?.attributes ?? [])
      .filter(isRelationshipFilterAttribute)
      .map((attribute) => ({
        ...attribute,
        code: `${directRelationships[index].code}.${attribute.code}`,
      })),
  );
  const secondTargets = useQueries({
    queries: secondRelationships.map((attribute) => ({
      queryKey: entityQueryKeys.blueprintByCode(
        attribute.target_blueprint_code,
        undefined,
      ),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getBlueprintByCode(attribute.target_blueprint_code, undefined, signal),
      enabled: discoverRelationshipPaths,
    })),
  });
  const thirdRelationships = secondTargets.flatMap((result, index) =>
    (result.data?.attributes ?? [])
      .filter(isRelationshipFilterAttribute)
      .map((attribute) => ({
        ...attribute,
        code: `${secondRelationships[index].code}.${attribute.code}`,
      })),
  );
  const relationshipPathsLoading =
    discoverRelationshipPaths &&
    [...firstTargets, ...secondTargets].some((result) => result.isFetching);
  const filterableAttributes = [
    ...new Map(
      [
        ...attributes.filter(isFilterableAttribute),
        ...pathAttributes.map(({ code, value_type }) => ({ code, value_type })),
        ...directRelationships,
        ...secondRelationships,
        ...thirdRelationships,
      ].map((attribute) => [attribute.code, attribute]),
    ).values(),
  ];
  const selectableAttributes =
    editingIndex === null
      ? filterableAttributes
      : filterableAttributes.filter(
          (item) => !isRelationshipFilterAttribute(item),
        );
  const attribute = filterableAttributes.find((item) => item.code === field);
  const availableOperators = operatorsForValueType(
    attribute?.value_type ?? 'string',
  );
  const effectiveOperator = availableOperators.includes(operator)
    ? operator
    : 'eq';
  const relationship = attribute && isRelationshipFilterAttribute(attribute);
  const maximumReached = filters.length >= maximumAttributeFilters;

  if (!filterableAttributes.length) {
    return (
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {t('explorer.noFilterableAttributes')}
      </Typography>
    );
  }

  const valueAsFilterValue = () => {
    if (!attribute) return value;
    if (attribute.value_type === 'number') return Number(value);
    if (attribute.value_type === 'integer') return Number.parseInt(value, 10);
    if (attribute.value_type === 'boolean') return value === 'true';
    if (attribute.value_type === 'datetime')
      return new Date(value).toISOString();
    return value;
  };
  const numericValue = Number(value);
  const valueIsValid =
    value !== '' &&
    (attribute?.value_type !== 'number' || Number.isFinite(numericValue)) &&
    (attribute?.value_type !== 'integer' ||
      (/^-?\d+$/.test(value) && Number.isSafeInteger(numericValue))) &&
    (attribute?.value_type !== 'datetime' ||
      !Number.isNaN(new Date(value).getTime()));
  const inputType =
    attribute?.value_type === 'number' || attribute?.value_type === 'integer'
      ? 'number'
      : attribute?.value_type === 'date'
        ? 'date'
        : attribute?.value_type === 'datetime'
          ? 'datetime-local'
          : attribute?.value_type === 'time'
            ? 'time'
            : 'text';
  const close = () => {
    setOpen(false);
    setEditingIndex(null);
    setField('');
    setOperator('eq');
    setValue('');
  };
  const openNewFilter = () => {
    setEditingIndex(null);
    setField('');
    setOperator('eq');
    setValue('');
    setOpen(true);
  };
  const openFilter = (filter: AttributeFilter, index: number) => {
    const filterAttribute = filterableAttributes.find(
      (item) => item.code === filter.field,
    );
    const inputValue = String(filter.value);
    const datetimeValue =
      filterAttribute?.value_type === 'datetime'
        ? new Date(
            new Date(inputValue).getTime() -
              new Date(inputValue).getTimezoneOffset() * 60_000,
          )
            .toISOString()
            .slice(0, 16)
        : inputValue;
    setEditingIndex(index);
    setField(filter.field);
    setOperator(filter.operator);
    setValue(datetimeValue);
    setOpen(true);
  };
  const apply = () => {
    if (!attribute) return;
    if (relationship) {
      onAddRelationship(attribute);
      close();
      return;
    }
    if (!valueIsValid || (maximumReached && editingIndex === null)) return;
    const filter = {
      field: attribute.code,
      operator: effectiveOperator,
      value: valueAsFilterValue(),
    };
    if (editingIndex === null) onAdd(filter);
    else onUpdate(editingIndex, filter);
    close();
  };
  const applyOnEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    apply();
  };
  const filterValueLabel = (filter: AttributeFilter) =>
    typeof filter.value === 'boolean'
      ? t(filter.value ? 'explorer.true' : 'explorer.false')
      : String(filter.value);

  return (
    <Stack spacing={1} sx={{ mt: 1 }}>
      <Stack
        direction="row"
        spacing={0.5}
        sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        useFlexGap
      >
        {filters.map((filter, index) => (
          <Chip
            key={`${filter.field}-${index}`}
            label={`${attributeLabel(
              filterableAttributes.find(
                (item) => item.code === filter.field,
              ) ?? { code: filter.field },
            )} ${t(
              `explorer.filterOperatorSymbols.${filter.operator}`,
            )} ${JSON.stringify(filterValueLabel(filter))}`}
            onClick={() => openFilter(filter, index)}
            onDelete={() => onRemove(index)}
            size="small"
          />
        ))}
        <Button
          color="primary"
          onClick={openNewFilter}
          size="small"
          startIcon={<AddIcon fontSize="small" />}
          sx={{
            borderRadius: 999,
            flexShrink: 0,
            height: 24,
            minHeight: 24,
            px: 1,
            '& .MuiButton-startIcon': { mr: 0.5 },
          }}
          variant="outlined"
        >
          {t('explorer.addAttributeFilter')}
        </Button>
      </Stack>
      {maximumReached && (
        <Typography color="text.secondary" variant="body2">
          {t('explorer.maximumAttributeFilters', {
            count: maximumAttributeFilters,
          })}
        </Typography>
      )}
      <RelationshipSelectorDialog
        actions={{
          applyDisabled:
            !attribute ||
            (!relationship &&
              (!valueIsValid || (maximumReached && editingIndex === null))),
          applyLabel: t(
            editingIndex === null
              ? 'explorer.applyAttributeFilter'
              : 'explorer.updateAttributeFilter',
          ),
          cancelLabel: t('explorer.cancelAttributeFilter'),
          clearLabel: t('explorer.clearAttributeFilter'),
          onApply: apply,
          onClear: () => setValue(''),
        }}
        closeLabel={t('explorer.closeAttributeFilter')}
        onClose={close}
        open={open}
        selectedLabel={t('explorer.attributeFilterDescription')}
        title={t(
          editingIndex === null
            ? 'explorer.addAttributeFilterTitle'
            : 'explorer.editAttributeFilterTitle',
        )}
      >
        <Stack spacing={1.5}>
          <TextField
            fullWidth
            label={t('explorer.filterField')}
            onChange={(event) => {
              const nextField = event.target.value;
              const nextAttribute = filterableAttributes.find(
                (item) => item.code === nextField,
              );
              if (
                nextAttribute &&
                isRelationshipFilterAttribute(nextAttribute)
              ) {
                onAddRelationship(nextAttribute);
                close();
                return;
              }
              setField(nextField);
              setOperator('eq');
              setValue('');
            }}
            select
            value={field}
          >
            {selectableAttributes.map((item) => (
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
          {relationshipPathsLoading && (
            <Typography color="text.secondary" variant="caption">
              {t('explorer.loadingRelationshipFields')}
            </Typography>
          )}
          {attribute && !relationship && (
            <>
              <TextField
                fullWidth
                label={t('explorer.operator')}
                onChange={(event) => {
                  const nextOperator = event.target.value;
                  const validOperator = availableOperators.find(
                    (item) => item === nextOperator,
                  );
                  if (validOperator) setOperator(validOperator);
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
              {attribute.value_type === 'boolean' ? (
                <TextField
                  fullWidth
                  label={t('explorer.value')}
                  onChange={(event) => setValue(event.target.value)}
                  onKeyDown={applyOnEnter}
                  select
                  value={value}
                >
                  <MenuItem value="true">{t('explorer.true')}</MenuItem>
                  <MenuItem value="false">{t('explorer.false')}</MenuItem>
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
                  onChange={(event) => setValue(event.target.value)}
                  onKeyDown={applyOnEnter}
                  type={inputType}
                  value={value}
                />
              )}
            </>
          )}
        </Stack>
      </RelationshipSelectorDialog>
    </Stack>
  );
};
