import { Button, CircularProgress } from '@mui/material';
import { useTranslation } from 'react-i18next';

const loadingIndicatorSize = 20;

export const LoadMoreButton = ({
  disabled = false,
  isLoading = false,
  onLoadMore,
}: {
  disabled?: boolean;
  isLoading?: boolean;
  onLoadMore: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <Button
      aria-busy={isLoading || undefined}
      aria-label={isLoading ? t('common.loading') : undefined}
      disabled={disabled || isLoading}
      fullWidth
      onClick={onLoadMore}
      size="medium"
      variant="contained"
    >
      {isLoading ? (
        <CircularProgress enableTrackSlot size={loadingIndicatorSize} />
      ) : (
        t('common.loadMore')
      )}
    </Button>
  );
};
