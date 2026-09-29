import { Box, IconButton, Tooltip } from '@mui/material';
import type { ReactNode } from 'react';
import { FunnelPlusIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { attributeFilterValueLabel } from './attributeFilters';
import type { AttributeFilter } from './search';

type Props = {
  fieldLabel: string;
  filter: AttributeFilter;
  onFilter: (filter: AttributeFilter) => void;
};

/** Revealed over its table cell while hovered or focused. */
export const CellFilterButton = ({ fieldLabel, filter, onFilter }: Props) => {
  const { t } = useTranslation();
  const label = t('explorer.filterByCellValue', {
    field: fieldLabel,
    value: attributeFilterValueLabel(t, filter),
  });

  return (
    <Tooltip title={label}>
      <IconButton
        aria-label={label}
        onClick={() => onFilter(filter)}
        size="small"
        sx={{
          bgcolor: 'background.paper',
          border: 1,
          borderColor: 'divider',
          opacity: 0,
          p: 0.25,
          position: 'absolute',
          right: 0,
          top: '50%',
          transform: 'translateY(-50%)',
          'td:hover &, &:focus-visible': { opacity: 1 },
          '&:hover': { bgcolor: 'background.paper' },
        }}
      >
        <FunnelPlusIcon size={compactIconSize} />
      </IconButton>
    </Tooltip>
  );
};

/** Adds the filter shortcut to a cell's content, when one applies. */
export const FilterableCell = ({
  children,
  ...props
}: Partial<Props> & { children: ReactNode }) =>
  props.filter && props.onFilter ? (
    // The shortcut overlays the cell edge so it never widens the column.
    <Box sx={{ position: 'relative' }}>
      {children}
      <CellFilterButton
        fieldLabel={props.fieldLabel ?? props.filter.field}
        filter={props.filter}
        onFilter={props.onFilter}
      />
    </Box>
  ) : (
    children
  );
