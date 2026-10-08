import { Box } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../../components/iconSizes';
import {
  valueTypeIcons,
  valueTypeLabelKey,
  type AttributeValueType,
} from '../valueTypeIcons';

type Props = {
  valueType: AttributeValueType;
};

const ValueTypeGlyph = ({ label, valueType }: Props & { label?: string }) => {
  const Icon = valueTypeIcons[valueType];

  return (
    <Box
      component="span"
      sx={{ color: 'text.secondary', display: 'inline-flex', flexShrink: 0 }}
    >
      {label ? (
        <Icon aria-label={label} role="img" size={compactIconSize} />
      ) : (
        <Icon aria-hidden size={compactIconSize} />
      )}
    </Box>
  );
};

/**
 * An attribute value type's icon, named for assistive technology. Use it where
 * the type is not otherwise written out, such as beside an attribute name.
 */
export const ValueTypeIcon = ({ valueType }: Props) => {
  const { t } = useTranslation();

  return (
    <ValueTypeGlyph
      label={t(valueTypeLabelKey(valueType))}
      valueType={valueType}
    />
  );
};

/** An attribute value type shown as its icon and translated name. */
export const ValueTypeLabel = ({ valueType }: Props) => {
  const { t } = useTranslation();

  return (
    <Box
      component="span"
      sx={{
        alignItems: 'center',
        display: 'inline-flex',
        gap: 1.5,
        whiteSpace: 'nowrap',
      }}
    >
      <ValueTypeGlyph valueType={valueType} />
      {t(valueTypeLabelKey(valueType))}
    </Box>
  );
};
