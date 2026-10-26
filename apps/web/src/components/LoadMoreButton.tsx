import {
  Button,
  CircularProgress,
  type SxProps,
  type Theme,
} from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

const loadingIndicatorSize = 20;
const compactLoadingIndicatorSize = 16;

/**
 * Loads the next page of a list. A compact button sits inside a panel or
 * tree as a small text button; otherwise it spans the list it extends.
 */
export const LoadMoreButton = ({
  compact = false,
  disabled = false,
  isLoading = false,
  label,
  onLoadMore,
  sx,
}: {
  compact?: boolean;
  disabled?: boolean;
  isLoading?: boolean;
  /** Names what loads, where a surface lists several kinds of items. */
  label?: ReactNode;
  onLoadMore: () => void;
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  return (
    <Button
      aria-busy={isLoading || undefined}
      aria-label={isLoading ? t('common.loading') : undefined}
      disabled={disabled || isLoading}
      fullWidth={!compact}
      onClick={onLoadMore}
      size={compact ? 'small' : 'medium'}
      sx={sx}
      variant={compact ? 'text' : 'contained'}
    >
      {isLoading ? (
        <CircularProgress
          enableTrackSlot
          size={compact ? compactLoadingIndicatorSize : loadingIndicatorSize}
        />
      ) : (
        (label ?? t('common.loadMore'))
      )}
    </Button>
  );
};
