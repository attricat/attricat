import { Chip, MenuItem } from '@mui/material';
import { CircleXIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { RuleFindingIcon } from '../../components/systemIcons';
import { capabilities } from '../auth/capabilities';
import { useCan } from '../auth/useCan';
import { dataQualityFilters, type DataQualityFilter } from './constants';
import { FilterMenuButton } from '../../components/FilterMenuButton';

/**
 * Filters Explorer results by their unresolved rule findings: a chip while
 * the filter applies, otherwise a menu of the filters. Only viewers who may
 * read rules see it.
 */
export const DataQualityFilterControl = ({
  value,
  onChange,
}: {
  value: DataQualityFilter | undefined;
  onChange: (value: DataQualityFilter | undefined) => void;
}) => {
  const { t } = useTranslation();
  const canReadRules = useCan(capabilities.rulesRead);
  if (!canReadRules) return null;
  if (value) {
    const label = t('explorer.dataQualityFilterChip', {
      filter: t(`explorer.dataQualityFilters.${value}`),
    });
    return (
      <Chip
        deleteIcon={
          <CircleXIcon
            aria-label={t('explorer.removeDataQualityFilter', {
              filter: label,
            })}
          />
        }
        label={label}
        size="small"
        title={label}
        onDelete={() => onChange(undefined)}
      />
    );
  }
  return (
    <FilterMenuButton icon={RuleFindingIcon} label={t('explorer.dataQuality')}>
      {(close) =>
        Object.values(dataQualityFilters).map((filter) => (
          <MenuItem
            key={filter}
            onClick={() => {
              close();
              onChange(filter);
            }}
          >
            {t(`explorer.dataQualityFilters.${filter}`)}
          </MenuItem>
        ))
      }
    </FilterMenuButton>
  );
};
