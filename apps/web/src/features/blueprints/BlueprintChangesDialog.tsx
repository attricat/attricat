import { Alert, Box, Stack, Typography } from '@mui/material';
import { lazy, Suspense, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { LoadingState } from '../../components/LoadingState';

const YamlDiffEditor = lazy(() =>
  import('../../components/YamlDiffEditor').then(({ YamlDiffEditor }) => ({
    default: YamlDiffEditor,
  })),
);

const codeList = (codes: readonly string[]) => codes.join(', ');

/**
 * What a new blueprint revision changes from the one it revises: the
 * attributes it adds and removes, and the definitions side by side, with
 * Save to keep it.
 */
export const BlueprintChangesDialog = ({
  checkedAttributeCodes,
  definition,
  error,
  isPending,
  onClose,
  onSave,
  open,
  saveLabel,
  source,
}: {
  /**
   * Attribute codes of the current definition, once it passes the save
   * checks; the summary waits for them.
   */
  checkedAttributeCodes?: readonly string[];
  definition: string;
  error: Error | null;
  isPending: boolean;
  onClose: () => void;
  onSave: () => void;
  open: boolean;
  saveLabel: ReactNode;
  source: {
    attributeCodes: readonly string[];
    definition: string;
    version: number;
  };
}) => {
  const { t } = useTranslation();
  const added = checkedAttributeCodes?.filter(
    (code) => !source.attributeCodes.includes(code),
  );
  const removed = checkedAttributeCodes
    ? source.attributeCodes.filter(
        (code) => !checkedAttributeCodes.includes(code),
      )
    : undefined;
  return (
    <ConfirmDialog
      confirmLabel={saveLabel}
      error={error}
      isPending={isPending}
      maxWidth="lg"
      onClose={onClose}
      onConfirm={onSave}
      open={open}
      title={t('blueprints.review.title', { version: source.version })}
    >
      <Stack spacing={2}>
        {added === undefined || removed === undefined ? (
          <Typography variant="body2">
            {t('blueprints.review.summaryPending')}
          </Typography>
        ) : (
          <>
            {added.length === 0 && removed.length === 0 && (
              <Typography variant="body2">
                {t('blueprints.review.sameAttributes')}
              </Typography>
            )}
            {added.length > 0 && (
              <Typography variant="body2">
                {t('blueprints.review.added', {
                  codes: codeList(added),
                  count: added.length,
                })}
              </Typography>
            )}
            {removed.length > 0 && (
              <Alert severity="warning">
                {t('blueprints.review.removed', {
                  codes: codeList(removed),
                  count: removed.length,
                })}
              </Alert>
            )}
          </>
        )}
        {open && (
          <Box>
            <Suspense
              fallback={
                <LoadingState>
                  {t('blueprints.review.loadingDiff')}
                </LoadingState>
              }
            >
              <YamlDiffEditor
                modified={definition}
                modifiedTitle={t('blueprints.review.unsaved')}
                original={source.definition}
                originalTitle={t('blueprints.review.sourceVersion', {
                  version: source.version,
                })}
              />
            </Suspense>
          </Box>
        )}
      </Stack>
    </ConfirmDialog>
  );
};
