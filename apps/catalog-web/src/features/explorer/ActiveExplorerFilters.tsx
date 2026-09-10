import CancelIcon from '@mui/icons-material/Cancel';
import { Box, Chip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { AttributeFilter } from './search';

type Props = {
  attributeFilters: AttributeFilter[];
  relationshipFilters: {
    field: string;
    labels?: string[];
    selectedCount: number;
  }[];
  onRemoveAttribute: (index: number) => void;
  onRemoveRelationship: (field: string) => void;
};

export const ActiveExplorerFilters = ({
  attributeFilters,
  relationshipFilters,
  onRemoveAttribute,
  onRemoveRelationship,
}: Props) => {
  const { t } = useTranslation();
  if (!attributeFilters.length && !relationshipFilters.length) return null;

  return (
    <Box
      aria-label={t('explorer.activeFilters')}
      sx={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: 1,
        mt: 1.25,
        minWidth: 0,
      }}
    >
      {relationshipFilters.map((filter) => {
        const label = filter.labels
          ? t('explorer.relationshipFilterLabel', {
              field: filter.field,
              value: filter.labels.join(', '),
            })
          : t('explorer.relationshipFilterPill', {
              field: filter.field,
              count: filter.selectedCount,
            });
        return (
          <Chip
            deleteIcon={
              <CancelIcon
                aria-label={t('explorer.removeRelationshipFilter', {
                  filter: label,
                })}
              />
            }
            key={filter.field}
            label={label}
            onDelete={() => onRemoveRelationship(filter.field)}
            size="small"
            sx={{
              maxWidth: '100%',
              '& .MuiChip-label': { overflow: 'hidden' },
            }}
            title={label}
          />
        );
      })}
      {attributeFilters.map((filter, index) => {
        const value =
          typeof filter.value === 'boolean'
            ? t(filter.value ? 'explorer.true' : 'explorer.false')
            : String(filter.value);
        const label = `${filter.field} ${t(
          `explorer.filterOperatorSymbols.${filter.operator}`,
        )} ${value}`;
        return (
          <Chip
            deleteIcon={
              <CancelIcon
                aria-label={t('explorer.removeAttributeFilter', {
                  filter: label,
                })}
              />
            }
            key={`${filter.field}-${filter.operator}-${String(filter.value)}-${index}`}
            label={label}
            onDelete={() => onRemoveAttribute(index)}
            size="small"
            sx={{
              maxWidth: '100%',
              '& .MuiChip-label': { overflow: 'hidden' },
            }}
            title={label}
          />
        );
      })}
    </Box>
  );
};
