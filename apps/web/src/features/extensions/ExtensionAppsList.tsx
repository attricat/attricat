import { Box, Button, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { OutletContribution } from './OutletContribution';
import {
  contributionKey,
  navigationByExtension,
  type OutletContext,
} from './outletContributions';

type ExtensionAppsListProps = {
  canBrowseExtensions: boolean;
  context?: OutletContext;
  contributions: ExtensionContribution[];
  onBrowseExtensions?: () => void;
  onNavigate?: () => void;
};

/** Lists every navigation contribution, grouped under its extension. */
export const ExtensionAppsList = ({
  canBrowseExtensions,
  context,
  contributions,
  onBrowseExtensions,
  onNavigate,
}: ExtensionAppsListProps) => {
  const { t } = useTranslation();
  const extensions = navigationByExtension(contributions);
  if (extensions.length === 0)
    return (
      <Box sx={{ px: 1, py: 1 }}>
        <Typography color="text.secondary" variant="body2">
          {t('extensions.noApps')}
        </Typography>
        {canBrowseExtensions && (
          <Button
            component={Link}
            onClick={() => {
              onBrowseExtensions?.();
              onNavigate?.();
            }}
            size="small"
            sx={{ mt: 1 }}
            to="/manage/extensions"
          >
            {t('extensions.browseExtensions')}
          </Button>
        )}
      </Box>
    );
  return (
    <Stack spacing={2}>
      {extensions.map(({ extensionId, extensionName, apps }) => (
        <Stack key={extensionId} spacing={0.5}>
          <Typography component="h2" variant="overline">
            {extensionName}
          </Typography>
          {apps.map((item) => (
            <OutletContribution
              contribution={item}
              context={context}
              key={contributionKey(item)}
              onNavigate={onNavigate}
            />
          ))}
        </Stack>
      ))}
    </Stack>
  );
};
