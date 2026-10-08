import { Box } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IconLabel } from '../../../components/IconLabel';
import { compactIconSize } from '../../../components/iconSizes';
import {
  valueTypeIcons,
  valueTypeLabelKey,
  type AttributeValueType,
} from '../valueTypeIcons';

type Props = {
  valueType: AttributeValueType;
};

/**
 * An attribute value type's icon, named for assistive technology. Use it where
 * the type is not otherwise written out, such as beside an attribute name.
 */
export const ValueTypeIcon = ({ valueType }: Props) => {
  const { t } = useTranslation();
  const Icon = valueTypeIcons[valueType];

  return (
    <Box
      component="span"
      sx={{ color: 'text.secondary', display: 'inline-flex', flexShrink: 0 }}
    >
      <Icon
        aria-label={t(valueTypeLabelKey(valueType))}
        role="img"
        size={compactIconSize}
      />
    </Box>
  );
};

/** An attribute value type shown as its icon and translated name. */
export const ValueTypeLabel = ({ valueType }: Props) => {
  const { t } = useTranslation();

  return (
    <IconLabel icon={valueTypeIcons[valueType]}>
      {t(valueTypeLabelKey(valueType))}
    </IconLabel>
  );
};
