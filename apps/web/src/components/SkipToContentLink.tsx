import { Button } from '@mui/material';
import { useTranslation } from 'react-i18next';

/** The id of the page's main landmark, which the skip link focuses. */
export const mainContentId = 'main-content';

/**
 * The first stop for the keyboard: it stays off screen until focused and
 * moves focus past the navigation to the page content.
 */
export const SkipToContentLink = () => {
  const { t } = useTranslation();
  return (
    <Button
      href={`#${mainContentId}`}
      onClick={(event) => {
        // Focusing directly keeps the router's URL free of a hash.
        event.preventDefault();
        document.getElementById(mainContentId)?.focus();
      }}
      sx={{
        left: (theme) => theme.spacing(4),
        position: 'fixed',
        top: (theme) => theme.spacing(4),
        transform: 'translateY(-200%)',
        zIndex: (theme) => theme.zIndex.tooltip,
        '&:focus-visible': { transform: 'none' },
      }}
      variant="contained"
    >
      {t('app.skipToContent')}
    </Button>
  );
};
