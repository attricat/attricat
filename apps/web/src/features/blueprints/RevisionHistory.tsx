import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import {
  Box,
  Button,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import type { Blueprint } from './api';
import { RevisionStatusChip } from '../../components/RevisionStatusChip';
import { Timestamp } from '../../time/Timestamp';
import { useCanOn } from '../auth/useCan';
import { capabilities, accessTargetKinds } from '../auth/capabilities';

export const RevisionHistory = ({
  blueprintId,
  revisions,
}: {
  blueprintId: string;
  revisions: Blueprint[];
}) => {
  const { t } = useTranslation();
  const canEdit = useCanOn(
    capabilities.blueprintsWrite,
    accessTargetKinds.blueprint,
    blueprintId,
  );
  return (
    <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
      <Typography component="h2" variant="h6">
        {t('blueprints.revisionHistory')}
      </Typography>
      <Box sx={{ overflowX: 'auto', mt: 1 }}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('blueprints.version')}</TableCell>
              <TableCell>{t('blueprints.status')}</TableCell>
              <TableCell>{t('blueprints.created')}</TableCell>
              <TableCell>{t('blueprints.published')}</TableCell>
              <TableCell>{t('blueprints.definitionHash')}</TableCell>
              <TableCell />
            </TableRow>
          </TableHead>
          <TableBody>
            {revisions.map((revision) => (
              <TableRow key={revision.version}>
                <TableCell>
                  {t('blueprints.versionNumber', {
                    version: revision.version,
                  })}
                </TableCell>
                <TableCell>
                  <RevisionStatusChip size="small" status={revision.status} />
                </TableCell>
                <TableCell>
                  <Timestamp
                    fallback={t('blueprints.notPublished')}
                    value={revision.created_at}
                  />
                </TableCell>
                <TableCell>
                  <Timestamp
                    fallback={t('blueprints.notPublished')}
                    value={revision.published_at}
                  />
                </TableCell>
                <TableCell sx={{ fontFamily: 'monospace' }}>
                  {revision.definition_hash}
                </TableCell>
                <TableCell align="right">
                  {canEdit && (
                    <Link
                      params={{
                        blueprintId,
                        version: String(revision.version),
                      }}
                      to="/manage/blueprints/$blueprintId/revisions/$version/new"
                    >
                      <Button size="small">{t('blueprints.edit')}</Button>
                    </Link>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Box>
    </Paper>
  );
};
