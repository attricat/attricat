import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createLink } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  Paper,
  Stack,
  Tab,
  Tabs,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import {
  acknowledgeFinding,
  listFindings,
  listRuleRuns,
  listRules,
  runRuleNow,
} from './api';
import { ruleQueryKeys } from './query-keys';

const RouterTab = createLink(Tab);

const sections = [
  { label: 'Rules', to: '/manage/rules' },
  { label: 'Findings', to: '/manage/rules/findings' },
  { label: 'Run history', to: '/manage/rules/runs' },
] as const;

export type RuleInspectionSection = 'rules' | 'findings' | 'runs';

export const RuleInspectionPage = ({
  section,
}: {
  section: RuleInspectionSection;
}) => {
  const queryClient = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead = session.data?.capabilities?.rules_read === true;
  const canManage = session.data?.capabilities?.rules_manage === true;
  const rules = useQuery({
    queryKey: ruleQueryKeys.definitions(),
    queryFn: listRules,
    enabled: canRead && section === 'rules',
  });
  const findings = useQuery({
    queryKey: ruleQueryKeys.findings(),
    queryFn: () => listFindings(),
    enabled: canRead && section === 'findings',
  });
  const runs = useQuery({
    queryKey: ruleQueryKeys.runs(),
    queryFn: listRuleRuns,
    enabled: canRead && section === 'runs',
  });
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: ruleQueryKeys.all });
  const acknowledge = useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: refresh,
  });
  const run = useMutation({
    mutationFn: ({ id, dryRun }: { id: string; dryRun: boolean }) =>
      runRuleNow(id, dryRun),
    onSuccess: refresh,
  });

  if (session.isPending) {
    return (
      <PageContainer>
        <Typography>Loading rules…</Typography>
      </PageContainer>
    );
  }

  if (!canRead) {
    return (
      <PageContainer>
        <Alert severity="error">
          You are not authorized to view data quality rules.
        </Alert>
      </PageContainer>
    );
  }

  const activeSectionIndex = {
    rules: 0,
    findings: 1,
    runs: 2,
  }[section];

  return (
    <PageContainer>
      <PageHeader
        title="Data quality rules"
        description="Inspect catalog-owned rule definitions, findings, and durable evaluation runs."
      />
      <Tabs
        aria-label="Rule inspection"
        sx={{ mt: 3 }}
        value={activeSectionIndex}
      >
        {sections.map((item, index) => (
          <RouterTab
            aria-controls={`rule-inspection-tabpanel-${index}`}
            id={`rule-inspection-tab-${index}`}
            key={item.to}
            label={item.label}
            to={item.to}
            value={index}
          />
        ))}
      </Tabs>
      <Box
        aria-labelledby={`rule-inspection-tab-${activeSectionIndex}`}
        id={`rule-inspection-tabpanel-${activeSectionIndex}`}
        role="tabpanel"
      >
        {section === 'rules' && (
          <RulesSection
            canManage={canManage}
            error={rules.isError}
            onRun={(id, dryRun) => run.mutate({ id, dryRun })}
            rules={rules.data}
          />
        )}
        {section === 'findings' && (
          <FindingsSection
            onAcknowledge={(id) => acknowledge.mutate(id)}
            canManage={canManage}
            error={findings.isError}
            findings={findings.data}
          />
        )}
        {section === 'runs' && (
          <RunsSection error={runs.isError} runs={runs.data} />
        )}
      </Box>
    </PageContainer>
  );
};

const RulesSection = ({
  canManage,
  error,
  onRun,
  rules,
}: {
  canManage: boolean;
  error: boolean;
  onRun: (id: string, dryRun: boolean) => void;
  rules: Awaited<ReturnType<typeof listRules>> | undefined;
}) => (
  <>
    {error && (
      <Alert severity="error" sx={{ mt: 3 }}>
        Unable to load rule definitions.
      </Alert>
    )}
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ overflowX: 'auto' }}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Rule</TableCell>
              <TableCell>Revision</TableCell>
              <TableCell>Lifecycle</TableCell>
              <TableCell>Actions</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(rules ?? []).map((rule) => (
              <TableRow key={`${rule.id}-${rule.version}`}>
                <TableCell>
                  <Stack spacing={0.25}>
                    <Typography>{rule.name}</Typography>
                    <Typography color="text.secondary" variant="caption">
                      {rule.code}
                    </Typography>
                  </Stack>
                </TableCell>
                <TableCell>v{rule.version}</TableCell>
                <TableCell>
                  <Chip
                    color={rule.enabled_version ? 'success' : 'default'}
                    label={rule.enabled_version ? 'Enabled' : 'Disabled'}
                    size="small"
                  />
                </TableCell>
                <TableCell>
                  {canManage && (
                    <>
                      <Button size="small" onClick={() => onRun(rule.id, true)}>
                        Dry run
                      </Button>
                      <Button
                        size="small"
                        onClick={() => onRun(rule.id, false)}
                      >
                        Run now
                      </Button>
                    </>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Box>
    </Paper>
  </>
);

const FindingsSection = ({
  canManage,
  error,
  findings,
  onAcknowledge,
}: {
  canManage: boolean;
  error: boolean;
  findings: Awaited<ReturnType<typeof listFindings>> | undefined;
  onAcknowledge: (id: string) => void;
}) => (
  <>
    {error && (
      <Alert severity="error" sx={{ mt: 3 }}>
        Unable to load rule findings.
      </Alert>
    )}
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ p: 2 }}>
        <Typography variant="h6">Active findings</Typography>
      </Box>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>Severity</TableCell>
            <TableCell>Finding</TableCell>
            <TableCell>State</TableCell>
            <TableCell />
          </TableRow>
        </TableHead>
        <TableBody>
          {(findings ?? [])
            .filter((item) => item.state !== 'resolved')
            .map((finding) => (
              <TableRow key={finding.id}>
                <TableCell>
                  <Chip
                    color={
                      finding.severity === 'error' ||
                      finding.severity === 'critical'
                        ? 'error'
                        : 'warning'
                    }
                    label={finding.severity}
                    size="small"
                  />
                </TableCell>
                <TableCell>{finding.message}</TableCell>
                <TableCell>{finding.state}</TableCell>
                <TableCell>
                  {canManage && finding.state === 'open' && (
                    <Button
                      size="small"
                      onClick={() => onAcknowledge(finding.id)}
                    >
                      Acknowledge
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
        </TableBody>
      </Table>
    </Paper>
  </>
);

const RunsSection = ({
  error,
  runs,
}: {
  error: boolean;
  runs: Awaited<ReturnType<typeof listRuleRuns>> | undefined;
}) => (
  <>
    {error && (
      <Alert severity="error" sx={{ mt: 3 }}>
        Unable to load rule run history.
      </Alert>
    )}
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ p: 2 }}>
        <Typography variant="h6">Run history</Typography>
      </Box>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>Source</TableCell>
            <TableCell>Status</TableCell>
            <TableCell>Evaluated</TableCell>
            <TableCell>Findings</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(runs ?? []).map((item) => (
            <TableRow key={item.id}>
              <TableCell>
                {item.source}
                {item.dry_run ? ' (dry run)' : ''}
              </TableCell>
              <TableCell>{item.status}</TableCell>
              <TableCell>{item.candidates_evaluated}</TableCell>
              <TableCell>{item.findings_created}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Paper>
  </>
);
