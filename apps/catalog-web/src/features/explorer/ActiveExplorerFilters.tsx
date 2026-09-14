import CancelIcon from '@mui/icons-material/Cancel';
import { Box, Chip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { AttributeFilter } from './search';

export type ActiveExplorerFilter =
  | {
      field: string;
      kind: 'relationship';
      labels?: string[];
      selectedCount: number;
    }
  | {
      filter: AttributeFilter;
      index: number;
      kind: 'attribute';
    };

type Props = {
  filters: ActiveExplorerFilter[];
  onRemoveAttribute: (index: number) => void;
  onRemoveRelationship: (field: string) => void;
};

export const ActiveExplorerFilters = ({
  filters,
  onRemoveAttribute,
  onRemoveRelationship,
}: Props) => {
  const { t } = useTranslation();
  if (!filters.length) return null;

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
      {filters.map((filter) => {
        const relationship = filter.kind === 'relationship';
        const value = relationship
          ? filter.labels
            ? filter.labels.map((label) => JSON.stringify(label)).join(', ')
            : undefined
          : typeof filter.filter.value === 'boolean'
            ? t(filter.filter.value ? 'explorer.true' : 'explorer.false')
            : String(filter.filter.value);
        const label = relationship
          ? value === undefined
            ? t('explorer.relationshipFilterPill', {
                field: filter.field,
                count: filter.selectedCount,
              })
            : t('explorer.relationshipFilterLabel', {
                field: filter.field,
                value,
              })
          : `${filter.filter.field} ${t(
              `explorer.filterOperatorSymbols.${filter.filter.operator}`,
            )} ${JSON.stringify(value)}`;
        return (
          <Chip
            deleteIcon={
              <CancelIcon
                aria-label={t(
                  relationship
                    ? 'explorer.removeRelationshipFilter'
                    : 'explorer.removeAttributeFilter',
                  { filter: label },
                )}
              />
            }
            key={
              relationship
                ? filter.field
                : `${filter.filter.field}-${filter.filter.operator}-${String(filter.filter.value)}-${filter.index}`
            }
            label={label}
            onDelete={() =>
              relationship
                ? onRemoveRelationship(filter.field)
                : onRemoveAttribute(filter.index)
            }
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
