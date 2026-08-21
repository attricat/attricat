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
    onClick={onLoadMore}
  >
    {isLoading ? 'Loading...' : 'Load more'}
  </Button>
);
