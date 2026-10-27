import {
  Box,
  Checkbox,
  Chip,
  IconButton,
  List,
  ListItem,
  Radio,
  Tooltip,
  Typography,
} from '@mui/material';
import { EyeIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

type Props = {
  isSample: boolean;
  /** Whether the row picks the one target (a radio) or toggles it (a checkbox). */
  isSingle: boolean;
  label: string;
  /** Name shared by the radios of one list. */
  name: string;
  onPreview: () => void;
  onSelect: () => void;
  previewHref: string;
  previewed: boolean;
  selected: boolean;
};

/**
 * Targets a relationship picker offers. A single-target list is a radio
 * group; otherwise each row is a checkbox.
 */
export const RelationshipTargetList = ({
  children,
  isSingle,
  label,
}: {
  children: ReactNode;
  isSingle: boolean;
  label: string;
}) => (
  <List
    aria-label={label}
    component="div"
    dense
    disablePadding
    role={isSingle ? 'radiogroup' : 'group'}
  >
    {children}
  </List>
);

/**
 * One relationship target: the whole row selects it, and the preview button
 * opens the record in a new tab.
 */
export const RelationshipTargetOption = ({
  isSample,
  isSingle,
  label,
  name,
  onPreview,
  onSelect,
  previewHref,
  previewed,
  selected,
}: Props) => {
  const { t } = useTranslation();
  const Control = isSingle ? Radio : Checkbox;
  return (
    <ListItem
      component="div"
      disablePadding
      secondaryAction={
        <Tooltip title={t('records.previewRelationshipOption')}>
          <IconButton
            aria-label={t('records.previewRelationshipOptionLabel', {
              option: label,
            })}
            color={previewed ? 'secondary' : 'default'}
            component="a"
            edge="end"
            href={previewHref}
            onAuxClick={onPreview}
            onClick={onPreview}
            rel="opener"
            size="small"
            target="_blank"
          >
            <EyeIcon />
          </IconButton>
        </Tooltip>
      }
    >
      <Box
        component="label"
        sx={{
          alignItems: 'center',
          bgcolor: selected
            ? 'action.selected'
            : previewed
              ? 'action.hover'
              : undefined,
          borderRadius: 1,
          cursor: 'pointer',
          display: 'flex',
          flexGrow: 1,
          gap: 1,
          minWidth: 0,
          pl: 0.5,
          pr: 6,
          '&:hover': { bgcolor: selected ? 'action.selected' : 'action.hover' },
        }}
      >
        <Control
          checked={selected}
          name={name}
          onChange={onSelect}
          size="small"
        />
        <Typography noWrap variant="body2">
          {label}
        </Typography>
        {isSample && (
          <Chip color="info" label={t('records.sample')} size="small" />
        )}
      </Box>
    </ListItem>
  );
};
