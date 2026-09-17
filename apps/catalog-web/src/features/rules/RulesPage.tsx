import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
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

const RULES_KEY = ['rules'] as const;
export const RulesPage = () => {
  const queryClient = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead = session.data?.capabilities?.rules_read === true;
  const canManage = session.data?.capabilities?.rules_manage === true;
  const rules = useQuery({
    queryKey: [...RULES_KEY, 'definitions'],
    queryFn: listRules,
    enabled: canRead,
  });
  const runs = useQuery({
    queryKey: [...RULES_KEY, 'runs'],
    queryFn: listRuleRuns,
    enabled: canRead,
  });
  const findings = useQuery({
    queryKey: [...RULES_KEY, 'findings'],
    queryFn: () => listFindings(),
    enabled: canRead,
  });
  const refresh = () => queryClient.invalidateQueries({ queryKey: RULES_KEY });
  const acknowledge = useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: refresh,
  });
  const run = useMutation({
    mutationFn: ({ id, dryRun }: { id: string; dryRun: boolean }) =>
      runRuleNow(id, dryRun),
    onSuccess: refresh,
  });
  if (session.isPending)
    return (
      <PageContainer>
        <Typography>Loading rules…</Typography>
      </PageContainer>
    );
  if (!canRead)
    return (
      <PageContainer>
        <Alert severity="error">
          You are not authorized to view data quality rules.
        </Alert>
      </PageContainer>
    );
  return (
    <PageContainer>
      <PageHeader
        title="Data quality rules"
        description="Inspect catalog-owned rule definitions, findings, and durable evaluation runs."
      />
      {(rules.isError || runs.isError || findings.isError) && (
        <Alert severity="error" sx={{ mt: 3 }}>
          Unable to load rule diagnostics.
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
              {(rules.data ?? []).map((rule) => (
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
                      label={rule.enabled_version ? 'Enabled' : 'Disabled'}
                      color={rule.enabled_version ? 'success' : 'default'}
                      size="small"
                    />
                  </TableCell>
                  <TableCell>
                    {canManage && (
                      <>
                        <Button
                          size="small"
                          onClick={() =>
                            run.mutate({ id: rule.id, dryRun: true })
                          }
                        >
                          Dry run
                        </Button>
                        <Button
                          size="small"
                          onClick={() =>
                            run.mutate({ id: rule.id, dryRun: false })
                          }
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
            {(findings.data ?? [])
              .filter((item) => item.state !== 'resolved')
              .map((finding) => (
                <TableRow key={finding.id}>
                  <TableCell>
                    <Chip
                      label={finding.severity}
                      color={
                        finding.severity === 'error' ||
                        finding.severity === 'critical'
                          ? 'error'
                          : 'warning'
                      }
                      size="small"
                    />
                  </TableCell>
                  <TableCell>{finding.message}</TableCell>
                  <TableCell>{finding.state}</TableCell>
                  <TableCell>
                    {canManage && finding.state === 'open' && (
                      <Button
                        size="small"
                        onClick={() => acknowledge.mutate(finding.id)}
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
            {(runs.data ?? []).map((item) => (
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
    </PageContainer>
  );
};
