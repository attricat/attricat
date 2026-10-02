import { useForm } from '@tanstack/react-form';
import {
  Box,
  Button,
  MenuItem,
  Paper,
  Popover,
  Stack,
  TextField,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Blueprint, BlueprintWithAttributes } from '../entities/api';
import {
  blueprintSelectWidth,
  pendingVersionPlaceholder,
  searchSyntaxPopoverMaxWidth,
  versionScopeSelectMinWidth,
  versionScopes,
} from './constants';
import { ExplorerQueryInput } from './ExplorerQueryInput';
import type { ExplorerSearch } from './search';
import { lexiconText } from '../lexicon/lexicon';

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
  /** Schema of the selected blueprint, used for query autocomplete. */
  blueprintSchema?: BlueprintWithAttributes;
  currentVersion?: number;
  revisions?: Blueprint[];
  revisionsError?: string;
  revisionsLoading?: boolean;
  search: ExplorerSearch;
  onRetryRevisions?: () => void;
  onSubmit: (value: ExplorerSearch) => void;
  lockedBlueprint?: boolean;
  /** Controls rendered before the search fields, such as saved searches. */
  startActions?: ReactNode;
  /** Controls rendered after the search button, such as sharing. */
  endActions?: ReactNode;
};

const inlineActionSx = { alignSelf: 'center', display: 'flex' } as const;

export const ExplorerSearchForm = ({
  blueprints,
  blueprintSchema,
  currentVersion,
  revisions = [],
  revisionsError,
  revisionsLoading = false,
  search,
  onRetryRevisions,
  onSubmit,
  lockedBlueprint = false,
  startActions,
  endActions,
}: Props) => {
  const { t } = useTranslation();
  const theme = useTheme();
  // Wide layouts keep the actions beside the search input; narrow layouts
  // move them above the stacked fields.
  const inlineActions = useMediaQuery(theme.breakpoints.up('md'));
  const [syntaxAnchor, setSyntaxAnchor] = useState<HTMLElement | null>(null);
  const [queryErrorContainer, setQueryErrorContainer] =
    useState<HTMLElement | null>(null);
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
      {!inlineActions && (startActions || endActions) && (
        <Stack direction="row" sx={{ justifyContent: 'space-between', mb: 1 }}>
          <span>{startActions}</span>
          <span>{endActions}</span>
        </Stack>
      )}
      <Stack direction={inlineActions ? 'row' : 'column'} spacing={1.5}>
        {inlineActions && startActions && (
          <Box sx={inlineActionSx}>{startActions}</Box>
        )}
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
                      name: lexiconText(blueprint.name),
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
            <ExplorerQueryInput
              blueprint={blueprintSchema}
              blueprints={blueprints}
              errorContainer={queryErrorContainer}
              onChange={field.handleChange}
              onShowSyntax={setSyntaxAnchor}
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button type="submit" variant="contained">
          {t('explorer.search')}
        </Button>
        {inlineActions && endActions && (
          <Box sx={inlineActionSx}>{endActions}</Box>
        )}
      </Stack>
      <div ref={setQueryErrorContainer} />
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
