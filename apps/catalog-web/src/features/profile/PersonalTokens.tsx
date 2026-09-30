import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Card,
  Chip,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import {
  BanIcon,
  CircleCheckIcon,
  ClockAlertIcon,
  Trash2Icon,
} from 'lucide-react';
import { Fragment, type ReactNode, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { smallIconSize } from '../../components/iconSizes';
import { PersonalTokenIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { listTokens, type PersonalToken } from './api';
import { tokenSkeletonCount, tokenSkeletonHeight } from './constants';
import { profileQueryKeys } from './queryKeys';
import { RevokeTokenDialog } from './RevokeTokenDialog';

type TokenStatus = 'active' | 'expired' | 'revoked';

const tokenStatus = (token: PersonalToken, now: Date): TokenStatus => {
  if (token.revoked_at) return 'revoked';
  if (token.expires_at && new Date(token.expires_at) <= now) return 'expired';
  return 'active';
};

const statusChips = {
  active: {
    color: 'success',
    icon: CircleCheckIcon,
    label: 'profile.tokenActive',
  },
  expired: {
    color: 'default',
    icon: ClockAlertIcon,
    label: 'profile.tokenExpired',
  },
  revoked: { color: 'default', icon: BanIcon, label: 'profile.tokenRevoked' },
} as const;

const tokenGridSx = {
  display: 'grid',
  gap: 4,
  gridTemplateColumns: { xs: '1fr', md: 'repeat(2, minmax(0, 1fr))' },
  listStyle: 'none',
  m: 0,
  p: 0,
};

const TokenCard = ({
  now,
  onRevoke,
  token,
}: {
  now: Date;
  onRevoke: () => void;
  token: PersonalToken;
}) => {
  const { t } = useTranslation();
  const status = tokenStatus(token, now);
  const chip = statusChips[status];
  const inactive = status !== 'active';
  const details: Array<[string, ReactNode]> = [
    [
      'profile.created',
      <Timestamp style="dateTime" value={token.created_at} />,
    ],
    [
      'profile.lastUsed',
      <Timestamp
        fallback={t('profile.never')}
        style="dateTime"
        value={token.last_used_at}
      />,
    ],
    [
      'profile.expires',
      <Timestamp
        fallback={t('profile.never')}
        style="dateTime"
        value={token.expires_at}
      />,
    ],
    ...(token.revoked_at
      ? [
          [
            'profile.revoked',
            <Timestamp style="dateTime" value={token.revoked_at} />,
          ] satisfies [string, ReactNode],
        ]
      : []),
  ];
  return (
    <Card
      component="li"
      sx={{ display: 'flex', flexDirection: 'column', gap: 4, p: 5 }}
    >
      <Stack direction="row" spacing={3} sx={{ alignItems: 'flex-start' }}>
        <Box
          sx={{
            color: inactive ? 'text.disabled' : 'text.secondary',
            display: 'flex',
            pt: 0.25,
          }}
        >
          <PersonalTokenIcon aria-hidden size={smallIconSize} />
        </Box>
        <Typography
          component="h3"
          sx={{ flex: 1, minWidth: 0, overflowWrap: 'anywhere' }}
          variant="subtitle1"
        >
          {token.label}
        </Typography>
        <Chip
          color={chip.color}
          icon={<chip.icon />}
          label={t(chip.label)}
          variant="outlined"
        />
      </Stack>
      <Box
        aria-label={t('profile.permissions')}
        component="ul"
        sx={{
          display: 'flex',
          flexWrap: 'wrap',
          gap: 1,
          listStyle: 'none',
          m: 0,
          p: 0,
        }}
      >
        {token.permissions.map((permission) => (
          <Chip
            component="li"
            key={permission}
            label={permission}
            sx={{ fontFamily: monoFontFamily, fontWeight: 400 }}
            variant="outlined"
          />
        ))}
      </Box>
      <Box
        component="dl"
        sx={{
          color: 'text.secondary',
          columnGap: 4,
          display: 'grid',
          gridTemplateColumns: 'max-content 1fr',
          m: 0,
          rowGap: 1,
          typography: 'caption',
          '& dd': { color: 'text.primary', m: 0 },
        }}
      >
        {details.map(([label, value]) => (
          <Fragment key={label}>
            <dt>{t(label)}</dt>
            <dd>{value}</dd>
          </Fragment>
        ))}
      </Box>
      {!token.revoked_at && (
        <Stack direction="row" sx={{ justifyContent: 'flex-end', mt: 'auto' }}>
          <Button
            color="error"
            onClick={onRevoke}
            startIcon={<Trash2Icon />}
            variant="outlined"
          >
            {t('profile.revoke')}
          </Button>
        </Stack>
      )}
    </Card>
  );
};

export const PersonalTokens = ({ canManage }: { canManage: boolean }) => {
  const { t } = useTranslation();
  const [revoking, setRevoking] = useState<PersonalToken>();
  const tokens = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokens(),
    queryFn: listTokens,
  });

  if (!canManage) {
    return <Alert severity="info">{t('profile.tokenUnavailable')}</Alert>;
  }

  const now = new Date();
  return (
    <Stack spacing={4}>
      <RevokeTokenDialog
        onClose={() => setRevoking(undefined)}
        token={revoking}
      />
      {tokens.isError && <Alert severity="error">{tokens.error.message}</Alert>}
      {tokens.isPending && (
        <Box sx={tokenGridSx}>
          {Array.from({ length: tokenSkeletonCount }, (_, index) => (
            <Skeleton
              height={tokenSkeletonHeight}
              key={index}
              variant="rounded"
            />
          ))}
        </Box>
      )}
      {tokens.data?.length === 0 && (
        <Card sx={{ p: 8, textAlign: 'center' }}>
          <Box sx={{ color: 'text.secondary' }}>
            <PersonalTokenIcon aria-hidden />
          </Box>
          <Typography component="h3" sx={{ mt: 2 }} variant="subtitle1">
            {t('profile.noTokens')}
          </Typography>
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            {t('profile.noTokensDescription')}
          </Typography>
        </Card>
      )}
      {Boolean(tokens.data?.length) && (
        <Box
          aria-label={t('profile.tokenList')}
          component="ul"
          sx={tokenGridSx}
        >
          {tokens.data?.map((token) => (
            <TokenCard
              key={token.id}
              now={now}
              onRevoke={() => setRevoking(token)}
              token={token}
            />
          ))}
        </Box>
      )}
    </Stack>
  );
};
