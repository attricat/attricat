import AddIcon from '@mui/icons-material/Add';
import {
  Button,
  Chip,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useMemo, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import type { Attribute } from '../entities/api';
import { attributeLabel } from '../entities/entity-display';
import {
  isFilterableAttribute,
  operatorsForValueType,
  type AttributeFilterOperator,
} from './attribute-filters';
import { maximumAttributeFilters, type AttributeFilter } from './search';

type Props = {
  filters: AttributeFilter[];
  attributes: Attribute[];
  onAdd: (filter: AttributeFilter) => void;
  onRemove: (index: number) => void;
  onUpdate: (index: number, filter: AttributeFilter) => void;
};

export const ExplorerAttributeFilters = ({
  filters,
  attributes,
  onAdd,
  onRemove,
  onUpdate,
}: Props) => {
  const { t } = useTranslation();
  const filterableAttributes = useMemo(
    () => attributes.filter(isFilterableAttribute),
    [attributes],
  );
  const [open, setOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [field, setField] = useState('');
  const [operator, setOperator] = useState<AttributeFilterOperator>('eq');
  const [value, setValue] = useState('');
  const attribute = filterableAttributes.find((item) => item.code === field);
  const availableOperators = operatorsForValueType(
    attribute?.value_type ?? 'string',
  );
  const effectiveOperator = availableOperators.includes(operator)
    ? operator
    : 'eq';
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
    if (
      !attribute ||
      !valueIsValid ||
      (maximumReached && editingIndex === null)
    )
      return;
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
          disabled={maximumReached}
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
        applyDisabled={
          !attribute ||
          !valueIsValid ||
          (maximumReached && editingIndex === null)
        }
        applyLabel={t(
          editingIndex === null
            ? 'explorer.applyAttributeFilter'
            : 'explorer.updateAttributeFilter',
        )}
        cancelLabel={t('explorer.cancelAttributeFilter')}
        clearLabel={t('explorer.clearAttributeFilter')}
        closeLabel={t('explorer.closeAttributeFilter')}
        onApply={apply}
        onClear={() => setValue('')}
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
            label={t('explorer.attribute')}
            onChange={(event) => {
              setField(event.target.value);
              setOperator('eq');
              setValue('');
            }}
            select
            value={field}
          >
            {filterableAttributes.map((item) => (
              <MenuItem key={item.code} value={item.code}>
                {attributeLabel(item)}
              </MenuItem>
            ))}
          </TextField>
          {attribute && (
            <>
              <TextField
                fullWidth
                label={t('explorer.operator')}
                onChange={(event) =>
                  setOperator(event.target.value as AttributeFilterOperator)
                }
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
