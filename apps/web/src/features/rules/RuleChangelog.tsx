import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Box,
  Chip,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { GitCompareArrowsIcon } from 'lucide-react';
import { lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadingState } from '../../components/LoadingState';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RevisionSelect } from '../../components/RevisionSelect';
import { RouterButton } from '../../components/RouterLink';
import { Timestamp } from '../../time/Timestamp';
import { principalKinds } from '../principals/constants';
import { principalReference } from '../principals/principal';
import { PrincipalText } from '../principals/PrincipalValue';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionEditor } from '../definition-editor/DefinitionEditor';
import type { RuleDetails, RuleRevision } from './api';
import { RULE_DEFINITION_VIEWER_HEIGHT, ruleRoutes } from './constants';
import { ruleRevisionsOptions } from './queryOptions';
import type { RuleComparisonSearch } from './schemas';

const YamlDiffEditor = lazy(() =>
  import('../../components/YamlDiffEditor').then(({ YamlDiffEditor }) => ({
    default: YamlDiffEditor,
  })),
);
const ignoreChange = () => undefined;

/**
 * The revisions to compare: those the URL names, else the shown revision and
 * the one saved before it. `to` defaults to the current revision. `from` is
 * undefined when `to` is the first revision.
 */
const comparedRevisions = (
  revisions: readonly RuleRevision[],
  { from, to }: RuleComparisonSearch,
) => {
  const byVersion = (version: number | undefined) =>
    revisions.find((revision) => revision.version === version);
  const right = byVersion(to) ?? revisions[0];
  const left =
    byVersion(from) ??
    revisions.find((revision) => revision.version < (right?.version ?? 0));
  return { left, right };
};

/** A rule's saved revisions, newest first, and a comparison of two of them. */
export const RuleChangelog = ({
  comparison,
  rule,
}: {
  comparison: RuleComparisonSearch;
  rule: RuleDetails;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const revisions = useQuery(ruleRevisionsOptions(rule.id));
  const items = revisions.data ?? [];
  const { left, right } = comparedRevisions(items, comparison);
  const label = (version: number) =>
    version === rule.version
      ? t('rules.changelog.currentRevisionOption', { version })
      : t('rules.revisionLabel', { version });
  const options = items.map(({ version }) => ({
    version,
    label: label(version),
  }));
  const compare = (search: RuleComparisonSearch) =>
    void navigate({
      params: { ruleId: rule.id },
      replace: true,
      search,
      to: ruleRoutes.changelog,
    });

  if (!revisions.data) {
    return revisions.isPending ? (
      <LoadingState>{t('rules.changelog.loading')}</LoadingState>
    ) : (
      <Box sx={{ mt: 3 }}>
        <QueryErrorNotice
          error={revisions.error}
          isRetrying={revisions.isFetching}
          onRetry={() => void revisions.refetch()}
        />
      </Box>
    );
  }

  return (
    <>
      <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
        <Typography component="h2" variant="h6">
          {t('rules.changelog.title')}
        </Typography>
        <Box sx={{ mt: 1, overflowX: 'auto' }}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('rules.columns.revision')}</TableCell>
                <TableCell>{t('rules.changelog.savedAt')}</TableCell>
                <TableCell>{t('rules.changelog.savedBy')}</TableCell>
                <TableCell>{t('rules.columns.actions')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {items.map((revision, index) => {
                const previous = items[index + 1];
                return (
                  <TableRow
                    key={revision.version}
                    selected={revision.version === right?.version}
                  >
                    <TableCell>
                      <Stack
                        direction="row"
                        spacing={1}
                        sx={{ alignItems: 'center' }}
                      >
                        <span>
                          {t('rules.revisionLabel', {
                            version: revision.version,
                          })}
                        </span>
                        {revision.version === rule.version && (
                          <Chip
                            color="primary"
                            label={t('rules.changelog.current')}
                            size="small"
                            variant="outlined"
                          />
                        )}
                      </Stack>
                    </TableCell>
                    <TableCell>
                      <Timestamp value={revision.created_at} />
                    </TableCell>
                    <TableCell>
                      {revision.created_by_user_id ? (
                        <PrincipalText
                          value={principalReference(
                            principalKinds.user,
                            revision.created_by_user_id,
                          )}
                        />
                      ) : (
                        <Typography color="text.secondary" variant="body2">
                          {t('rules.changelog.unknownAuthor')}
                        </Typography>
                      )}
                    </TableCell>
                    <TableCell>
                      {previous && (
                        <RouterButton
                          params={{ ruleId: rule.id }}
                          replace
                          search={{
                            from: previous.version,
                            to: revision.version,
                          }}
                          size="small"
                          startIcon={<GitCompareArrowsIcon />}
                          to={ruleRoutes.changelog}
                        >
                          {t('rules.changelog.comparePrevious')}
                        </RouterButton>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Box>
      </Paper>
      {right && (
        <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
          <Typography component="h2" variant="h6">
            {t('rules.changelog.compareTitle')}
          </Typography>
          <Typography color="text.secondary" sx={{ mt: 1 }}>
            {t('rules.changelog.compareDescription')}
          </Typography>
          <Stack
            direction={{ xs: 'column', sm: 'row' }}
            spacing={2}
            sx={{ mt: 2 }}
          >
            <RevisionSelect
              label={t('rules.changelog.from')}
              onChange={(version) =>
                compare({ from: version, to: right.version })
              }
              options={options}
              value={left?.version}
            />
            <RevisionSelect
              label={t('rules.changelog.to')}
              onChange={(version) =>
                compare({ from: left?.version, to: version })
              }
              options={options}
              value={right.version}
            />
          </Stack>
          <Box sx={{ mt: 3 }}>
            {left ? (
              <Suspense
                fallback={
                  <LoadingState>
                    {t('rules.changelog.loadingDiff')}
                  </LoadingState>
                }
              >
                <YamlDiffEditor
                  modified={right.definition}
                  modifiedTitle={label(right.version)}
                  original={left.definition}
                  originalTitle={label(left.version)}
                />
              </Suspense>
            ) : (
              <>
                <Typography color="text.secondary" variant="body2">
                  {t('rules.changelog.firstRevision')}
                </Typography>
                <DefinitionEditor
                  ariaLabel={label(right.version)}
                  height={RULE_DEFINITION_VIEWER_HEIGHT}
                  kind={definitionKinds.rule}
                  marginTop={1}
                  onChange={ignoreChange}
                  readOnly
                  value={right.definition}
                />
              </>
            )}
          </Box>
        </Paper>
      )}
    </>
  );
};
