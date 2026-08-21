import { Button } from '@mui/material';

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
    {isLoading ? 'Loading...' : 'Load more'}
  </Button>
);
