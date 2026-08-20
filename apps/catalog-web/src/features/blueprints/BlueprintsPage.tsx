import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Chip,
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
import { useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { listBlueprints } from './api';
import { blueprintQueryKeys } from './query-keys';

const dateTime = (value: string | null) =>
  value
    ? new Intl.DateTimeFormat(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(new Date(value))
    : 'Not published';

export const BlueprintsPage = () => {
  const [query, setQuery] = useState('');
  const blueprints = useQuery({
    queryKey: blueprintQueryKeys.catalogue(),
    queryFn: listBlueprints,
  });
  const matchingBlueprints = (blueprints.data ?? []).filter((blueprint) => {
    const term = query.toLowerCase();
    return (
      blueprint.code.toLowerCase().includes(term) ||
      blueprint.name.toLowerCase().includes(term) ||
      blueprint.kind.toLowerCase().includes(term) ||
      blueprint.status.toLowerCase().includes(term)
    );
  });

  return (
    <PageContainer>
      <PageHeader
        description="Inspect every entity and mixin blueprint, including unpublished revisions."
        title="Blueprints"
      />
      <TextField
        label="Filter blueprints"
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Name, code, kind, or status"
        sx={{ mt: 3, width: { xs: '100%', sm: 420 } }}
        value={query}
      />
      {blueprints.isPending && (
        <Typography sx={{ mt: 3 }}>Loading blueprints...</Typography>
      )}
      {blueprints.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {blueprints.error.message}
        </Alert>
      )}
      {blueprints.data && (
        <Paper component="section" sx={{ mt: 3 }}>
          <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
            <Typography>
              {matchingBlueprints.length} blueprint
              {matchingBlueprints.length === 1 ? '' : 's'}
            </Typography>
          </Box>
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Blueprint</TableCell>
                  <TableCell>Kind</TableCell>
                  <TableCell>Latest version</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Published</TableCell>
                  <TableCell>Updated</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {matchingBlueprints.map((blueprint) => (
                  <TableRow hover key={blueprint.id}>
                    <TableCell>
                      <Stack spacing={0.25}>
                        <Link
                          params={{ blueprintId: blueprint.id }}
                          to="/blueprints/$blueprintId"
                        >
                          {blueprint.name}
                        </Link>
                        <Typography color="text.secondary" variant="caption">
                          {blueprint.code}
                        </Typography>
                      </Stack>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={blueprint.kind}
                        size="small"
                        variant="outlined"
                      />
                    </TableCell>
                    <TableCell>v{blueprint.version}</TableCell>
                    <TableCell>
                      <Chip
                        color={
                          blueprint.status === 'published'
                            ? 'success'
                            : 'warning'
                        }
                        label={blueprint.status}
                        size="small"
                      />
                    </TableCell>
                    <TableCell>{dateTime(blueprint.published_at)}</TableCell>
                    <TableCell>{dateTime(blueprint.updated_at)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
          {matchingBlueprints.length === 0 && (
            <Typography sx={{ p: 2 }}>
              No blueprints matched this filter.
            </Typography>
          )}
        </Paper>
      )}
    </PageContainer>
  );
};
