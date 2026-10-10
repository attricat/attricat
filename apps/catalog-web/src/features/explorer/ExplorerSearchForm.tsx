import { useForm } from '@tanstack/react-form';
import {
  Badge,
  Box,
  Button,
  IconButton,
  Link,
  MenuItem,
  Paper,
  Popover,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import {
  useEffect,
  useId,
  useState,
  type ReactNode,
  type RefObject,
} from 'react';
import { useTranslation } from 'react-i18next';
import { ExternalLinkIcon, RotateCcwIcon, SearchIcon } from 'lucide-react';
import { useDocumentationUrl } from '../../app/documentation';
import { smallIconSize } from '../../components/iconSizes';
import { SearchScopeIcon } from '../../components/systemIcons';
import type { AttributeContext } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import type { Blueprint, BlueprintWithAttributes } from '../records/api';
import {
  blueprintSelectWidth,
  pendingVersionPlaceholder,
  searchScopePopoverWidth,
  searchSyntaxPopoverMaxWidth,
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
  /** Applies a blueprint choice at once instead of on submit. */
  onBlueprintChange?: (blueprint: string) => void;
  contexts?: AttributeContext[];
  contextCode?: string;
  onContextChange?: (contextCode: string) => void;
  /** Controls rendered before the search fields, such as saved searches. */
  startActions?: ReactNode;
  /** Controls rendered after the search button, such as sharing. */
  endActions?: ReactNode;
  queryInputRef?: RefObject<HTMLInputElement | null>;
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
  onBlueprintChange,
  contexts = [],
  contextCode = defaultContextCode,
  onContextChange,
  startActions,
  endActions,
  queryInputRef,
}: Props) => {
  const { t } = useTranslation();
  const documentationUrl = useDocumentationUrl();
  const theme = useTheme();
  // Wide layouts keep the actions beside the search input; narrow layouts
  // move them above the stacked fields.
  const inlineActions = useMediaQuery(theme.breakpoints.up('md'));
  const [syntaxAnchor, setSyntaxAnchor] = useState<HTMLElement | null>(null);
  const [scopeAnchor, setScopeAnchor] = useState<HTMLElement | null>(null);
  const scopePopoverId = useId();
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

  const historicalRevisions = revisions
    .filter((revision) => revision.version !== currentVersion)
    .sort((left, right) => right.version - left.version);
  const contextLabel = (code: string) =>
    code === defaultContextCode ? t('explorer.default') : code;
  const contextChoice = onContextChange && contexts.length > 1;
  const requestedScope = searchFormValues(search).versionScope;
  // Naming the current version explicitly is still the default scope.
  const versionScope =
    requestedScope === String(currentVersion)
      ? versionScopes.current
      : requestedScope;
  const versionLabel =
    versionScope === versionScopes.current
      ? t('explorer.currentVersion', {
          version: currentVersion ?? pendingVersionPlaceholder,
        })
      : versionScope === versionScopes.all
        ? t('explorer.allVersions')
        : t('explorer.version', { version: versionScope });
  const scopeLabel = t('explorer.searchScopeSummary', {
    scope: contextChoice
      ? t('explorer.searchScopeParts', {
          context: contextLabel(contextCode),
          version: versionLabel,
        })
      : versionLabel,
  });
  // Highlighted whenever the scope narrows or widens what a plain search shows.
  const scopeChanged =
    versionScope !== versionScopes.current ||
    contextCode !== defaultContextCode;
  const scopeButton = search.blueprint && (
    <Tooltip title={scopeLabel}>
      <IconButton
        aria-controls={scopeAnchor ? scopePopoverId : undefined}
        aria-expanded={Boolean(scopeAnchor)}
        aria-haspopup="true"
        aria-label={scopeLabel}
        color={revisionsError ? 'error' : scopeChanged ? 'primary' : 'default'}
        onClick={(event) => setScopeAnchor(event.currentTarget)}
        sx={[
          { alignSelf: 'center', flexShrink: 0 },
          scopeChanged && {
            bgcolor: 'action.selected',
            '&:hover': { bgcolor: 'action.focus' },
          },
        ]}
      >
        {/* The dot marks a non-default scope; the label announces it. */}
        <Badge
          aria-hidden
          color={revisionsError ? 'error' : 'primary'}
          invisible={!scopeChanged && !revisionsError}
          overlap="circular"
          variant="dot"
        >
          <SearchScopeIcon size={smallIconSize} />
        </Badge>
      </IconButton>
    </Tooltip>
  );

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
      {!inlineActions && (startActions || endActions || scopeButton) && (
        <Stack direction="row" sx={{ justifyContent: 'space-between', mb: 1 }}>
          <Stack direction="row" spacing={0.5}>
            {startActions}
            {scopeButton}
          </Stack>
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
                onChange={(event) => {
                  field.handleChange(event.target.value);
                  onBlueprintChange?.(event.target.value);
                }}
                select
                sx={{ flexShrink: 0, width: blueprintSelectWidth }}
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
        {inlineActions && scopeButton}
        <form.Field name="query">
          {(field) => (
            <ExplorerQueryInput
              blueprint={blueprintSchema}
              blueprints={blueprints}
              errorContainer={queryErrorContainer}
              onChange={field.handleChange}
              onShowSyntax={setSyntaxAnchor}
              queryInputRef={queryInputRef}
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button
          startIcon={<SearchIcon />}
          sx={{ flexShrink: 0 }}
          type="submit"
          variant="contained"
        >
          {t('explorer.search')}
        </Button>
        {inlineActions && endActions && (
          <Box sx={inlineActionSx}>{endActions}</Box>
        )}
      </Stack>
      <div ref={setQueryErrorContainer} />
      <Popover
        anchorEl={scopeAnchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        onClose={() => setScopeAnchor(null)}
        open={Boolean(scopeAnchor) && Boolean(search.blueprint)}
        slotProps={{
          paper: {
            id: scopePopoverId,
            sx: { p: 2, width: searchScopePopoverWidth },
          },
        }}
      >
        <Stack spacing={2}>
          <form.Field name="versionScope">
            {(field) => (
              <TextField
                disabled={revisionsLoading}
                error={Boolean(revisionsError)}
                fullWidth
                helperText={revisionsError}
                label={t('explorer.versionScope')}
                onChange={(event) => {
                  const versionScope = event.target.value;
                  field.handleChange(versionScope);
                  submitValues({ ...form.state.values, versionScope });
                  setScopeAnchor(null);
                }}
                select
                value={field.state.value}
              >
                <MenuItem value={versionScopes.current}>
                  {t('explorer.currentVersion', {
                    version: currentVersion ?? pendingVersionPlaceholder,
                  })}
                </MenuItem>
                {historicalRevisions.map((revision) => (
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
          {revisionsError && onRetryRevisions && (
            <Button
              onClick={onRetryRevisions}
              startIcon={<RotateCcwIcon />}
              sx={{ alignSelf: 'start' }}
              variant="text"
            >
              {t('explorer.retry')}
            </Button>
          )}
          {/* Only worth choosing once there is more than the default. */}
          {contextChoice && (
            <TextField
              fullWidth
              label={t('explorer.context')}
              onChange={(event) => {
                onContextChange(event.target.value);
                setScopeAnchor(null);
              }}
              select
              value={contextCode}
            >
              {contexts.map((context) => (
                <MenuItem key={context.id} value={context.code}>
                  {contextLabel(context.code)}
                </MenuItem>
              ))}
            </TextField>
          )}
        </Stack>
      </Popover>
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
        <Link
          href={documentationUrl('searchSyntax')}
          rel="noopener noreferrer"
          sx={{
            alignItems: 'center',
            display: 'inline-flex',
            gap: 0.5,
            mt: 1.5,
          }}
          target="_blank"
          variant="body2"
        >
          {t('explorer.searchSyntaxGuide')}
          <ExternalLinkIcon aria-hidden size={smallIconSize} />
        </Link>
      </Popover>
    </Paper>
  );
};
