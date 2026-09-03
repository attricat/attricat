import { Link } from '@tanstack/react-router';
import {
  Box,
  Button,
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import type { Blueprint } from './api';
import { formatBlueprintDateTime } from './date-time';

export const RevisionHistory = ({
  blueprintId,
  revisions,
}: {
  blueprintId: string;
  revisions: Blueprint[];
}) => (
  <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
    <Typography component="h2" variant="h6">
      Revision history
    </Typography>
    <Box sx={{ overflowX: 'auto', mt: 1 }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>Version</TableCell>
            <TableCell>Status</TableCell>
            <TableCell>Created</TableCell>
            <TableCell>Published</TableCell>
            <TableCell>Definition hash</TableCell>
            <TableCell />
          </TableRow>
        </TableHead>
        <TableBody>
          {revisions.map((revision) => (
            <TableRow key={revision.version}>
              <TableCell>v{revision.version}</TableCell>
              <TableCell>
                <Chip
                  color={
                    revision.status === 'published' ? 'success' : 'warning'
                  }
                  label={revision.status}
                  size="small"
                />
              </TableCell>
              <TableCell>
                {formatBlueprintDateTime(revision.created_at)}
              </TableCell>
              <TableCell>
                {formatBlueprintDateTime(revision.published_at)}
              </TableCell>
              <TableCell sx={{ fontFamily: 'monospace' }}>
                {revision.definition_hash}
              </TableCell>
              <TableCell align="right">
                <Link
                  params={{ blueprintId, version: String(revision.version) }}
                  to="/manage/blueprints/$blueprintId/revisions/$version/new"
                >
                  <Button size="small">Edit</Button>
                </Link>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  </Paper>
);
