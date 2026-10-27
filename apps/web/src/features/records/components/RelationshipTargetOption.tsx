import { Box, Button, Chip, Link, ListItem, Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';

type Props = {
  isSample: boolean;
  label: string;
  onPreview: () => void;
  onPreviewLinkClick: () => void;
  onSelect: () => void;
  previewHref: string;
  previewed: boolean;
  /**
   * Whether the target is already chosen, for pickers where the button
   * toggles it; without it the button only selects.
   */
  selected?: boolean;
};

/** One selectable relationship target in a relationship picker. */
export const RelationshipTargetOption = ({
  isSample,
  label,
  onPreview,
  onPreviewLinkClick,
  onSelect,
  previewHref,
  previewed,
  selected = false,
}: Props) => {
  const { t } = useTranslation();
  return (
    <ListItem
      sx={{
        alignItems: 'center',
        bgcolor: previewed ? 'action.selected' : undefined,
        borderRadius: 1,
        '&:hover': { bgcolor: 'action.hover' },
      }}
    >
      <Box
        sx={{
          alignItems: 'center',
          display: 'flex',
          flexGrow: 1,
          gap: 1,
          minWidth: 0,
          mr: 1,
        }}
      >
        <Link
          href={previewHref}
          onClick={onPreviewLinkClick}
          rel="opener"
          target="_blank"
        >
          {label}
        </Link>
        {isSample && (
          <Chip color="info" label={t('records.sample')} size="small" />
        )}
      </Box>
      <Stack direction="row" spacing={0.5}>
        <Button
          aria-label={t('records.previewRelationshipOptionLabel', {
            option: label,
          })}
          color={previewed ? 'secondary' : 'inherit'}
          onClick={onPreview}
          size="small"
        >
          {t('records.previewRelationshipOption')}
        </Button>
        <Button
          aria-label={t(
            selected
              ? 'records.removeRelationshipOptionLabel'
              : 'records.selectRelationshipOptionLabel',
            { option: label },
          )}
          onClick={onSelect}
          size="small"
        >
          {selected
            ? t('records.removeRelationshipOption')
            : t('records.selectRelationshipOption')}
        </Button>
      </Stack>
    </ListItem>
  );
};
