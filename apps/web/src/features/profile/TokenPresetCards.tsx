import { Box, Card, CardActionArea, Stack, Typography } from '@mui/material';
import { CircleCheckIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { tokenPermissionPresets } from './constants';

const sameCodes = (left: readonly string[], right: readonly string[]) =>
  left.length === right.length && left.every((code) => right.includes(code));

/**
 * Preset permission sets for a token; the one matching `selected` is marked,
 * and presets naming a permission the user cannot grant are disabled.
 */
export const TokenPresetCards = ({
  available,
  onSelect,
  selected,
}: {
  /** Permissions the user can grant, or `undefined` while they load. */
  available: ReadonlySet<string> | undefined;
  onSelect: (permissions: string[]) => void;
  selected: readonly string[];
}) => {
  const { t } = useTranslation();
  return (
    <Box
      sx={{
        display: 'grid',
        gap: 3,
        gridTemplateColumns: {
          xs: '1fr',
          sm: 'repeat(3, minmax(0, 1fr))',
        },
        mt: 3,
      }}
    >
      {tokenPermissionPresets.map((preset) => {
        const unavailable = available
          ? preset.permissions.some((code) => !available.has(code))
          : false;
        const active = sameCodes(selected, preset.permissions);
        return (
          <Card
            key={preset.nameKey}
            sx={{
              borderColor: active ? 'primary.main' : undefined,
              bgcolor: active ? 'action.selected' : undefined,
            }}
          >
            <CardActionArea
              aria-pressed={active}
              disabled={!available || unavailable}
              onClick={() => onSelect([...preset.permissions])}
              sx={{ height: '100%', p: 4 }}
            >
              <Stack
                direction="row"
                spacing={2}
                sx={{
                  alignItems: 'center',
                  justifyContent: 'space-between',
                }}
              >
                <Typography
                  color={unavailable ? 'text.disabled' : undefined}
                  variant="subtitle1"
                >
                  {t(preset.nameKey)}
                </Typography>
                {active && (
                  <Box sx={{ color: 'primary.main', display: 'flex' }}>
                    <CircleCheckIcon aria-hidden size={compactIconSize} />
                  </Box>
                )}
              </Stack>
              <Typography
                color="text.secondary"
                component="p"
                sx={{ mt: 1 }}
                variant="caption"
              >
                {unavailable
                  ? t('profile.unavailable')
                  : t(preset.descriptionKey)}
              </Typography>
              <Typography
                color="text.secondary"
                component="p"
                sx={{ mt: 2 }}
                variant="caption"
              >
                {t('profile.permissionCount', {
                  count: preset.permissions.length,
                })}
              </Typography>
            </CardActionArea>
          </Card>
        );
      })}
    </Box>
  );
};
