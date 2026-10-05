import { Box, Chip } from '@mui/material';
import { CircleXIcon } from 'lucide-react';
import { useLayoutEffect, useRef, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import { principalConfiguration } from '../principals/principal';
import { usePrincipalDirectory } from '../principals/usePrincipalDirectory';
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
  /** The selected blueprint's attributes, to label fields and values. */
  attributes?: Attribute[];
  filters: ActiveExplorerFilter[];
  /** Receives focus when the last filter is removed and the row disappears. */
  emptyFocusTarget?: RefObject<HTMLElement | null>;
  onRemoveAttribute: (index: number) => void;
  onRemoveRelationship: (field: string) => void;
};

export const ActiveExplorerFilters = ({
  attributes = [],
  filters,
  emptyFocusTarget,
  onRemoveAttribute,
  onRemoveRelationship,
}: Props) => {
  const { t } = useTranslation();
  const chipRefs = useRef<(HTMLDivElement | null)[]>([]);
  const removedPosition = useRef<number | null>(null);
  // Deleting a chip removes the focused element. Move focus to the chip that
  // took its place, else the previous one, else the target outside the row.
  useLayoutEffect(() => {
    const position = removedPosition.current;
    if (position === null) return;
    removedPosition.current = null;
    const next = chipRefs.current[Math.min(position, filters.length - 1)];
    (next ?? emptyFocusTarget?.current)?.focus();
  }, [emptyFocusTarget, filters.length]);
  const findAttribute = (code: string) =>
    attributes.find((item) => item.code === code);
  const directory = usePrincipalDirectory(
    filters.some((filter) => {
      if (filter.kind !== 'attribute') return false;
      const attribute = findAttribute(filter.filter.field);
      return Boolean(attribute && principalConfiguration(attribute));
    }),
  );
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
      {filters.map((filter, position) => {
        const relationship = filter.kind === 'relationship';
        const label = relationship
          ? t('explorer.relationshipFilterPill', {
              field: filter.field,
              count: filter.selectedCount,
            })
          : attributeFilterLabel(
              t,
              filter.filter,
              findAttribute(filter.filter.field),
              directory.data,
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
            onDelete={() => {
              removedPosition.current = position;
              if (relationship) onRemoveRelationship(filter.field);
              else onRemoveAttribute(filter.index);
            }}
            ref={(element: HTMLDivElement | null) => {
              chipRefs.current[position] = element;
            }}
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
