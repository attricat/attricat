import { Button } from '@mui/material';
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
    disabled={disabled || isLoading}
    fullWidth
    onClick={onLoadMore}
    size="medium"
    variant="contained"
  >
    {isLoading ? i18n.t('common.loading') : i18n.t('common.loadMore')}
  </Button>
);
