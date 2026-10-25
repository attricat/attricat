import { Alert, Box, Button } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useCanOn } from '../../auth/useCan';
import type { Attribute, StatusTransitionAccess } from '../api';
import { attributeLabel } from '../recordDisplay';
import { statusCodeLabel, statusOptionLabel, type StatusLock } from '../status';
import { UnlockStatusDialog } from './UnlockStatusDialog';
import { capabilities, accessTargetKinds } from '../../auth/capabilities';

type StatusLockNoticeProps = {
  locks: readonly StatusLock[];
  /** Attributes shown on the record, to name the fields a partial lock covers. */
  attributes: readonly Attribute[];
  /** Server-evaluated access to edges leaving the saved statuses. */
  transitions: readonly StatusTransitionAccess[];
  recordId: string;
  contextId: string | null;
  /** The record version an unlock is checked against, once it is loaded. */
  updatedAt?: string;
};

/**
 * Explains why fields cannot be edited: which status locks them and which
 * transitions, if any, the user can make to unlock them. Owners and
 * administrators can also unlock outside those transitions.
 */
export const StatusLockNotice = ({
  locks,
  attributes,
  transitions,
  recordId,
  contextId,
  updatedAt,
}: StatusLockNoticeProps) => {
  const { t, i18n } = useTranslation();
  const canUnlock =
    useCanOn(capabilities.recordsUnlock, accessTargetKinds.record, recordId) &&
    updatedAt !== undefined;
  const [unlocking, setUnlocking] = useState<string>();
  if (locks.length === 0) return null;
  const list = (items: string[], type: Intl.ListFormatType) =>
    new Intl.ListFormat(i18n.language, { type }).format(items);
  const allowedTargets = (lock: StatusLock) =>
    transitions
      .filter(
        (edge) =>
          edge.attribute_code === lock.attribute.code &&
          edge.from === lock.option.code &&
          edge.to !== null &&
          edge.allowed,
      )
      .flatMap((edge) => (edge.to === null ? [] : [edge.to]));
  const unlockingLock = locks.find((lock) => lock.attribute.code === unlocking);

  return (
    <>
      <Alert severity="info" sx={{ mb: 2 }}>
        {locks.map((lock) => {
          const { attribute, config, option, lockedCodes } = lock;
          const status = attributeLabel(attribute);
          const value = statusOptionLabel(option);
          const locked =
            option.lock === 'all'
              ? t('records.statusLock.all', {
                  attribute: status,
                  status: value,
                })
              : t('records.statusLock.some', {
                  attribute: status,
                  status: value,
                  fields: list(
                    lockedCodes.map((code) => {
                      const field = attributes.find(
                        (item) => item.code === code,
                      );
                      return field ? attributeLabel(field) : code;
                    }),
                    'conjunction',
                  ),
                });
          const declared = transitions.some(
            (edge) =>
              edge.attribute_code === attribute.code &&
              edge.from === option.code &&
              edge.to !== null,
          );
          const targets = allowedTargets(lock).map(
            (code) => statusCodeLabel(config, code) ?? code,
          );
          const unlock =
            targets.length > 0
              ? t('records.statusLock.unlockBy', {
                  attribute: status,
                  targets: list(targets, 'disjunction'),
                })
              : canUnlock
                ? null
                : declared
                  ? t('records.statusLock.unlockNotPermitted')
                  : t('records.statusLock.unlockNone');
          return (
            <Box
              key={attribute.code}
              sx={{
                alignItems: 'center',
                display: 'flex',
                flexWrap: 'wrap',
                gap: 1,
                justifyContent: 'space-between',
              }}
            >
              <Box component="p" sx={{ m: 0 }}>
                {locked}
                {unlock && ` ${unlock}`}
              </Box>
              {canUnlock && (
                <Button
                  color="inherit"
                  onClick={() => setUnlocking(attribute.code)}
                  size="small"
                  variant="outlined"
                >
                  {t('records.statusLock.unlockAction')}
                </Button>
              )}
            </Box>
          );
        })}
      </Alert>
      {unlockingLock && updatedAt && (
        <UnlockStatusDialog
          contextId={contextId}
          lock={unlockingLock}
          onClose={() => setUnlocking(undefined)}
          recordId={recordId}
          suggested={allowedTargets(unlockingLock)[0]}
          updatedAt={updatedAt}
        />
      )}
    </>
  );
};
