import { useForm } from '@tanstack/react-form';
import { Button, MenuItem, Paper, Stack, TextField } from '@mui/material';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { Blueprint } from '../entities/api';
import type { ExplorerSearch } from './search';

type Props = {
  blueprints: Blueprint[];
  currentVersion?: number;
  revisions?: Blueprint[];
  revisionsError?: string;
  revisionsLoading?: boolean;
  search: ExplorerSearch;
  onRetryRevisions?: () => void;
  onSubmit: (value: ExplorerSearch) => void;
  lockedBlueprint?: boolean;
};

export const ExplorerSearchForm = ({
  blueprints,
  currentVersion,
  revisions = [],
  revisionsError,
  revisionsLoading = false,
  search,
  onRetryRevisions,
  onSubmit,
  lockedBlueprint = false,
}: Props) => {
  const { t } = useTranslation();
  const submitValues = (value: {
    blueprint: string;
    query: string;
    versionScope: string;
  }) => {
    const historicalVersion = Number(value.versionScope);
    onSubmit({
      blueprint: value.blueprint || undefined,
      ...(value.versionScope === 'all'
        ? { allVersions: true }
        : Number.isInteger(historicalVersion)
          ? { version: historicalVersion }
          : {}),
      query: value.query || undefined,
    });
  };
  const form = useForm({
    defaultValues: {
      blueprint: search.blueprint ?? '',
      query: search.query ?? '',
      versionScope: search.allVersions
        ? 'all'
        : search.version === undefined
          ? 'current'
          : String(search.version),
    },
    onSubmit: ({ value }) => submitValues(value),
  });

  useEffect(() => {
    form.reset({
      blueprint: search.blueprint ?? '',
      query: search.query ?? '',
      versionScope: search.allVersions
        ? 'all'
        : search.version === undefined
          ? 'current'
          : String(search.version),
    });
  }, [
    form,
    search.allVersions,
    search.blueprint,
    search.query,
    search.version,
  ]);

  return (
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      sx={{
        mt: 2.5,
        p: 1.5,
      }}
    >
      <Stack direction={{ xs: 'column', md: 'row' }} spacing={1.5}>
        {!lockedBlueprint && (
          <form.Field name="blueprint">
            {(field) => (
              <TextField
                required
                label={t('explorer.selectBlueprint')}
                onChange={(event) => field.handleChange(event.target.value)}
                select
                sx={{ width: 280 }}
                value={field.state.value}
              >
                {blueprints.map((blueprint) => (
                  <MenuItem key={blueprint.code} value={blueprint.code}>
                    {blueprint.name} ({blueprint.code})
                  </MenuItem>
                ))}
              </TextField>
            )}
          </form.Field>
        )}
        {search.blueprint && (
          <form.Field name="versionScope">
            {(field) => (
              <TextField
                disabled={revisionsLoading}
                error={Boolean(revisionsError)}
                helperText={revisionsError}
                label={t('explorer.versionScope')}
                onChange={(event) => {
                  const versionScope = event.target.value;
                  field.handleChange(versionScope);
                  submitValues({ ...form.state.values, versionScope });
                }}
                select
                sx={{ minWidth: 190 }}
                value={field.state.value}
              >
                <MenuItem value="current">
                  {t('explorer.currentVersion', {
                    version: currentVersion ?? '…',
                  })}
                </MenuItem>
                {revisions
                  .filter((revision) => revision.version !== currentVersion)
                  .sort((left, right) => right.version - left.version)
                  .map((revision) => (
                    <MenuItem
                      key={revision.version}
                      value={String(revision.version)}
                    >
                      {t('explorer.version', { version: revision.version })}
                    </MenuItem>
                  ))}
                <MenuItem value="all">{t('explorer.allVersions')}</MenuItem>
              </TextField>
            )}
          </form.Field>
        )}
        {revisionsError && onRetryRevisions && (
          <Button onClick={onRetryRevisions} variant="text">
            {t('common.retry')}
          </Button>
        )}
        <form.Field name="query">
          {(field) => (
            <TextField
              fullWidth
              label={t('explorer.query')}
              onChange={(event) => field.handleChange(event.target.value)}
              helperText={t('explorer.queryExamples')}
              placeholder={t('explorer.searchTerms')}
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button type="submit" variant="contained">
          {t('explorer.search')}
        </Button>
      </Stack>
    </Paper>
  );
};
