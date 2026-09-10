import AddIcon from '@mui/icons-material/Add';
import { Button, MenuItem, Stack, TextField, Typography } from '@mui/material';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import {
  isFilterableAttribute,
  operatorsForValueType,
  type AttributeFilterOperator,
} from './attribute-filters';
import { maximumAttributeFilters, type AttributeFilter } from './search';

type Props = {
  activeFilterCount: number;
  attributes: Attribute[];
  onAdd: (filter: AttributeFilter) => void;
};

export const ExplorerAttributeFilters = ({
  activeFilterCount,
  attributes,
  onAdd,
}: Props) => {
  const { t } = useTranslation();
  const filterableAttributes = useMemo(
    () => attributes.filter(isFilterableAttribute),
    [attributes],
  );
  const [field, setField] = useState('');
  const [operator, setOperator] = useState<AttributeFilterOperator>('eq');
  const [value, setValue] = useState('');
  const effectiveField = filterableAttributes.some(
    (item) => item.code === field,
  )
    ? field
    : (filterableAttributes[0]?.code ?? '');
  const attribute = filterableAttributes.find(
    (item) => item.code === effectiveField,
  );
  const availableOperators = operatorsForValueType(
    attribute?.value_type ?? 'string',
  );
  const effectiveOperator = availableOperators.includes(operator)
    ? operator
    : 'eq';

  const maximumReached = activeFilterCount >= maximumAttributeFilters;

  if (!filterableAttributes.length) {
    return (
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {t('explorer.noFilterableAttributes')}
      </Typography>
    );
  }

  const addFilter = () => {
    if (!attribute || value === '') return;
    let parsedValue: string | number | boolean = value;
    if (attribute.value_type === 'number') parsedValue = Number(value);
    if (attribute.value_type === 'integer')
      parsedValue = Number.parseInt(value, 10);
    if (attribute.value_type === 'boolean') parsedValue = value === 'true';
    if (attribute.value_type === 'datetime')
      parsedValue = new Date(value).toISOString();
    onAdd({
      field: effectiveField,
      operator: effectiveOperator,
      value: parsedValue,
    });
    setValue('');
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

  return (
    <Stack spacing={1} sx={{ mt: 1 }}>
      <TextField
        fullWidth
        label={t('explorer.attribute')}
        onChange={(event) => {
          setField(event.target.value);
          setOperator('eq');
          setValue('');
        }}
        select
        size="small"
        value={effectiveField}
      >
        {filterableAttributes.map((item) => (
          <MenuItem key={item.code} value={item.code}>
            {item.code}
          </MenuItem>
        ))}
      </TextField>
      <TextField
        fullWidth
        label={t('explorer.operator')}
        onChange={(event) =>
          setOperator(event.target.value as AttributeFilterOperator)
        }
        select
        size="small"
        value={effectiveOperator}
      >
        {availableOperators.map((item) => (
          <MenuItem key={item} value={item}>
            {t(`explorer.filterOperators.${item}`)}
          </MenuItem>
        ))}
      </TextField>
      {attribute?.value_type === 'boolean' ? (
        <TextField
          fullWidth
          label={t('explorer.value')}
          onChange={(event) => setValue(event.target.value)}
          select
          size="small"
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
              attribute?.value_type === 'integer' ? { step: 1 } : undefined,
          }}
          label={t('explorer.value')}
          onChange={(event) => setValue(event.target.value)}
          size="small"
          type={inputType}
          value={value}
        />
      )}
      {maximumReached && (
        <Typography color="text.secondary" variant="body2">
          {t('explorer.maximumAttributeFilters', {
            count: maximumAttributeFilters,
          })}
        </Typography>
      )}
      <Button
        disabled={!valueIsValid || maximumReached}
        fullWidth
        onClick={addFilter}
        startIcon={<AddIcon />}
        variant="outlined"
      >
        {t('explorer.applyAttributeFilter')}
      </Button>
    </Stack>
  );
};
