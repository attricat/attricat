import { Box, Card, CardActionArea, Stack, Typography } from '@mui/material';
import { CircleCheckIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import {
  allTokenPermissions,
  tokenPermissionPresets,
  type TokenPermissionPreset,
} from './constants';

const sameCodes = (left: readonly string[], right: readonly string[]) =>
  left.length === right.length && left.every((code) => right.includes(code));

/**
 * Preset permission sets for a token; the one matching `selected` is marked,
 * and presets naming a permission the user cannot grant are disabled. The
 * support preset selects every permission the user can grant except those
 * that would outlast the token.
 */
export const TokenPresetCards = ({
  available,
  onSelect,
  selected,
}: {
  /** Permissions the user can grant, or `undefined` while they load. */
  available: ReadonlySet<string> | undefined;
  onSelect: (permissions: string[], preset: TokenPermissionPreset) => void;
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
          sm: 'repeat(2, minmax(0, 1fr))',
        },
        mt: 3,
      }}
    >
      {tokenPermissionPresets.map((preset) => {
        const permissions =
          preset.permissions === allTokenPermissions
            ? [...(available ?? [])].filter(
                (code) => !preset.excludedPermissions?.includes(code),
              )
            : preset.permissions;
        const unavailable = available
          ? permissions.length === 0 ||
            permissions.some((code) => !available.has(code))
          : false;
        const active =
          available !== undefined && sameCodes(selected, permissions);
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
              onClick={() => onSelect([...permissions], preset)}
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
                  count: permissions.length,
                })}
              </Typography>
            </CardActionArea>
          </Card>
        );
      })}
    </Box>
  );
};
