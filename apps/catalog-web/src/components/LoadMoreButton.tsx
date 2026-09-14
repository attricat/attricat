import { Button, CircularProgress } from '@mui/material';
import i18n from '../i18n';

export const LoadMoreButton = ({
  disabled = false,
  isLoading = false,
  onLoadMore,
}: {
  disabled?: boolean;
  isLoading?: boolean;
  onLoadMore: () => void;
}) => (
  <Button
    aria-busy={isLoading || undefined}
    aria-label={isLoading ? i18n.t('common.loading') : undefined}
    disabled={disabled || isLoading}
    fullWidth
    onClick={onLoadMore}
    size="medium"
    variant="contained"
  >
    {isLoading ? (
      <CircularProgress enableTrackSlot size={20} />
    ) : (
      i18n.t('common.loadMore')
    )}
  </Button>
);
