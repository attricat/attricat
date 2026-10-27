import {
  Button,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clearBudget, listBudgets, setBudget } from './api';
import { extensionExportQueryKeys } from './queryKeys';
import type { HostPermissionBudget } from './schemas';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

const BudgetRow = ({
  budget,
  canManage,
  extensionId,
}: {
  budget: HostPermissionBudget;
  canManage: boolean;
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const declared = budget.declared;
  const [requests, setRequests] = useState(
    String(budget.lowered?.requests ?? declared?.requests ?? ''),
  );
  const [perSeconds, setPerSeconds] = useState(
    String(budget.lowered?.per_seconds ?? declared?.per_seconds ?? ''),
  );
  const refresh = () =>
    client.invalidateQueries({
      queryKey: extensionExportQueryKeys.budgets(extensionId),
    });
  const save = useMutation({
    mutationFn: () =>
      setBudget(extensionId, budget.host_permission_id, {
        requests: Number(requests),
        per_seconds: Number(perSeconds),
      }),
    onSuccess: refresh,
  });
  const reset = useMutation({
    mutationFn: () => clearBudget(extensionId, budget.host_permission_id),
    onSuccess: () => {
      // The inputs go back to the declared budget the reset restores.
      setRequests(String(declared?.requests ?? ''));
      setPerSeconds(String(declared?.per_seconds ?? ''));
      return refresh();
    },
  });
  const describe = (limit: { requests: number; per_seconds: number }) =>
    t('extensionExports.budgetRate', {
      requests: limit.requests,
      seconds: limit.per_seconds,
    });
  return (
    <TableRow>
      <TableCell>{budget.host_permission_id}</TableCell>
      <TableCell>
        {declared ? describe(declared) : t('extensionExports.sharedBudget')}
      </TableCell>
      <TableCell>
        {budget.lowered ? describe(budget.lowered) : '—'}
        <ApiErrorAlert error={save.error ?? reset.error} sx={{ mt: 1 }} />
      </TableCell>
      <TableCell>
        {canManage && declared && (
          <Stack
            component="form"
            direction="row"
            spacing={1}
            onSubmit={(event) => {
              event.preventDefault();
              save.mutate();
            }}
          >
            <TextField
              label={t('extensionExports.requests')}
              size="small"
              type="number"
              value={requests}
              onChange={(event) => setRequests(event.target.value)}
            />
            <TextField
              label={t('extensionExports.perSeconds')}
              size="small"
              type="number"
              value={perSeconds}
              onChange={(event) => setPerSeconds(event.target.value)}
            />
            <Button disabled={save.isPending} type="submit">
              {t('extensionExports.lower')}
            </Button>
            {budget.lowered && (
              <Button disabled={reset.isPending} onClick={() => reset.mutate()}>
                {t('extensionExports.resetBudget')}
              </Button>
            )}
          </Stack>
        )}
      </TableCell>
    </TableRow>
  );
};

/** Request budgets of the extension's host permissions; operators may lower them. */
export const ExtensionBudgetsSection = ({
  canManage,
  extensionId,
}: {
  canManage: boolean;
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const budgets = useQuery({
    queryKey: extensionExportQueryKeys.budgets(extensionId),
    queryFn: () => listBudgets(extensionId),
  });
  if (budgets.isError) return <ApiErrorAlert error={budgets.error} />;
  if (!budgets.data?.length) return null;
  return (
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensionExports.budgets')}</Typography>
      <Typography color="text.secondary" sx={{ mb: 1 }} variant="body2">
        {t('extensionExports.budgetsDescription')}
      </Typography>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('extensionExports.hostPermission')}</TableCell>
            <TableCell>{t('extensionExports.declaredBudget')}</TableCell>
            <TableCell>{t('extensionExports.loweredBudget')}</TableCell>
            <TableCell />
          </TableRow>
        </TableHead>
        <TableBody>
          {budgets.data.map((budget) => (
            <BudgetRow
              budget={budget}
              canManage={canManage}
              extensionId={extensionId}
              key={budget.host_permission_id}
            />
          ))}
        </TableBody>
      </Table>
    </Paper>
  );
};
