import { useForm } from '@tanstack/react-form';
import {
  Button,
  IconButton,
  InputAdornment,
  MenuItem,
  Paper,
  Popover,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { CircleQuestionMarkIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Blueprint } from '../entities/api';
import {
  blueprintSelectWidth,
  pendingVersionPlaceholder,
  searchSyntaxPopoverMaxWidth,
  versionScopeSelectMinWidth,
  versionScopes,
} from './constants';
import type { ExplorerSearch } from './search';
import { smallIconSize } from '../../components/iconSizes';

type SearchFormValues = {
  blueprint: string;
  query: string;
  versionScope: string;
};

const searchFormValues = (search: ExplorerSearch): SearchFormValues => ({
  blueprint: search.blueprint ?? '',
  query: search.query ?? '',
  versionScope: search.allVersions
    ? versionScopes.all
    : search.version === undefined
      ? versionScopes.current
      : String(search.version),
});

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
  const [syntaxAnchor, setSyntaxAnchor] = useState<HTMLElement | null>(null);
  const submitValues = (value: SearchFormValues) => {
    const historicalVersion = Number(value.versionScope);
    onSubmit({
      blueprint: value.blueprint || undefined,
      ...(value.versionScope === versionScopes.all
        ? { allVersions: true }
        : Number.isInteger(historicalVersion)
          ? { version: historicalVersion }
          : {}),
      query: value.query || undefined,
    });
  };
  const form = useForm({
    defaultValues: searchFormValues(search),
    onSubmit: ({ value }) => submitValues(value),
  });

  useEffect(() => {
    form.reset(
      searchFormValues({
        allVersions: search.allVersions,
        blueprint: search.blueprint,
        query: search.query,
        version: search.version,
      }),
    );
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
                sx={{ width: blueprintSelectWidth }}
                value={field.state.value}
              >
                {blueprints.map((blueprint) => (
                  <MenuItem key={blueprint.code} value={blueprint.code}>
                    {t('explorer.blueprintOption', {
                      code: blueprint.code,
                      name: blueprint.name,
                    })}
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
                sx={{ minWidth: versionScopeSelectMinWidth }}
                value={field.state.value}
              >
                <MenuItem value={versionScopes.current}>
                  {t('explorer.currentVersion', {
                    version: currentVersion ?? pendingVersionPlaceholder,
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
                <MenuItem value={versionScopes.all}>
                  {t('explorer.allVersions')}
                </MenuItem>
              </TextField>
            )}
          </form.Field>
        )}
        {revisionsError && onRetryRevisions && (
          <Button onClick={onRetryRevisions} variant="text">
            {t('explorer.retry')}
          </Button>
        )}
        <form.Field name="query">
          {(field) => (
            <TextField
              fullWidth
              label={t('explorer.query')}
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder={t('explorer.searchTerms')}
              slotProps={{
                input: {
                  endAdornment: (
                    <InputAdornment position="end">
                      <Tooltip title={t('explorer.searchSyntax')}>
                        <IconButton
                          aria-label={t('explorer.searchSyntax')}
                          onClick={(event) =>
                            setSyntaxAnchor(event.currentTarget)
                          }
                          size="small"
                          type="button"
                        >
                          <CircleQuestionMarkIcon size={smallIconSize} />
                        </IconButton>
                      </Tooltip>
                    </InputAdornment>
                  ),
                },
              }}
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button type="submit" variant="contained">
          {t('explorer.search')}
        </Button>
      </Stack>
      <Popover
        anchorEl={syntaxAnchor}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        onClose={() => setSyntaxAnchor(null)}
        open={Boolean(syntaxAnchor)}
        slotProps={{
          paper: { sx: { maxWidth: searchSyntaxPopoverMaxWidth, p: 2 } },
        }}
        transformOrigin={{ horizontal: 'right', vertical: 'top' }}
      >
        <Typography variant="body2">{t('explorer.queryExamples')}</Typography>
      </Popover>
    </Paper>
  );
};
