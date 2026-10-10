import { Button, Link, ListItem, Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';

type Props = {
  href: string;
  label: string;
  onOpenPreview: () => void;
  onPreviewLinkClick: () => void;
  onToggle: () => void;
  previewed: boolean;
  selected: boolean;
};

export const RelationshipTargetOption = ({
  href,
  label,
  onOpenPreview,
  onPreviewLinkClick,
  onToggle,
  previewed,
  selected,
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
      <Link
        href={href}
        onClick={onPreviewLinkClick}
        rel="opener"
        sx={{ flexGrow: 1, minWidth: 0, mr: 1 }}
        target="_blank"
      >
        {label}
      </Link>
      <Stack direction="row" spacing={0.5}>
        <Button
          aria-label={t('records.previewRelationshipOptionLabel', {
            option: label,
          })}
          color={previewed ? 'secondary' : 'inherit'}
          onClick={onOpenPreview}
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
          onClick={onToggle}
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
