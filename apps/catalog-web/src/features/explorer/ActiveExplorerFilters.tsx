import { Box, Chip } from '@mui/material';
import { CircleXIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import { attributeFilterKey, attributeFilterLabel } from './attributeFilters';
import type { AttributeFilter } from './search';

export type ActiveExplorerFilter =
  | {
      field: string;
      kind: 'relationship';
      selectedCount: number;
    }
  | {
      filter: AttributeFilter;
      index: number;
      kind: 'attribute';
    };

type Props = {
  /** The selected blueprint's attributes, to show status labels. */
  attributes?: Attribute[];
  filters: ActiveExplorerFilter[];
  onRemoveAttribute: (index: number) => void;
  onRemoveRelationship: (field: string) => void;
};

export const ActiveExplorerFilters = ({
  attributes = [],
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
        const label = relationship
          ? t('explorer.relationshipFilterPill', {
              field: filter.field,
              count: filter.selectedCount,
            })
          : attributeFilterLabel(
              t,
              filter.filter,
              filter.filter.field,
              attributes.find((item) => item.code === filter.filter.field),
            );
        return (
          <Chip
            deleteIcon={
              <CircleXIcon
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
                : attributeFilterKey(filter.filter, filter.index)
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
