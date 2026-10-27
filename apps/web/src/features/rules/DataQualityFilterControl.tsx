import { Button, Chip, Menu, MenuItem } from '@mui/material';
import { CircleXIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import { smallIconSize } from '../../components/iconSizes';
import { RuleFindingIcon } from '../../components/systemIcons';
import { capabilities } from '../auth/capabilities';
import { useCan } from '../auth/useCan';
import { dataQualityFilters, type DataQualityFilter } from './constants';

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
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
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
    <>
      <Button
        color="primary"
        size="small"
        startIcon={<RuleFindingIcon size={smallIconSize} />}
        sx={compactOutlinedActionButtonSx}
        variant="outlined"
        onClick={(event) => setAnchor(event.currentTarget)}
      >
        {t('explorer.dataQuality')}
      </Button>
      <Menu
        anchorEl={anchor}
        open={Boolean(anchor)}
        onClose={() => setAnchor(null)}
      >
        {Object.values(dataQualityFilters).map((filter) => (
          <MenuItem
            key={filter}
            onClick={() => {
              setAnchor(null);
              onChange(filter);
            }}
          >
            {t(`explorer.dataQualityFilters.${filter}`)}
          </MenuItem>
        ))}
      </Menu>
    </>
  );
};
