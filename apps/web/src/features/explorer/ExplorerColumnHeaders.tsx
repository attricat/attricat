import { ButtonBase, TableSortLabel, Tooltip } from '@mui/material';
import { use } from 'react';
import { useTranslation } from 'react-i18next';
import { sortDirections } from './constants';
import {
  configuredColumnLabel,
  type ExplorerTableColumn,
} from './explorerTableColumns';
import { ExplorerSortContext } from './explorerSortContext';

type SortableHeaderProps = {
  field: string;
  label: string;
  onSortChange: (field: string) => void;
};

export const SortableColumnHeader = ({
  field,
  label,
  onSortChange,
}: SortableHeaderProps) => {
  const sort = use(ExplorerSortContext);
  const active = sort?.field === field;
  return (
    <TableSortLabel
      active={active}
      direction={active ? sort.direction : sortDirections.ascending}
      onClick={() => onSortChange(field)}
    >
      {label}
    </TableSortLabel>
  );
};

type ConfiguredHeaderProps = {
  column: ExplorerTableColumn;
  onSortChange: (field: string) => void;
};

export const ConfiguredColumnHeader = ({
  column,
  onSortChange,
}: ConfiguredHeaderProps) => {
  const { t } = useTranslation();
  const label = configuredColumnLabel(column);
  if (column.relationshipSortBlocked) {
    const reason = t('explorer.relationshipSortNeedsSingleVersion');
    return (
      <Tooltip title={reason}>
        <ButtonBase
          aria-disabled="true"
          aria-label={t('explorer.unsortableColumnLabel', {
            column: label,
            reason,
          })}
          disableRipple
          sx={{ cursor: 'help', font: 'inherit' }}
        >
          {label}
        </ButtonBase>
      </Tooltip>
    );
  }
  if (!column.sortable) return label;
  return (
    <SortableColumnHeader
      field={column.field}
      label={label}
      onSortChange={onSortChange}
    />
  );
};
