import {
  Box,
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { monospaceFontFamily, workflowStatus } from './constants';
import { Timestamp } from '../../time/Timestamp';
import type { Workflow } from './schemas';

export const WorkflowRevisionTable = ({
  revisions,
}: {
  revisions: Workflow[];
}) => {
  const { t } = useTranslation();
  return (
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ overflowX: 'auto' }}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('workflows.currentRevision')}</TableCell>
              <TableCell>{t('workflows.status')}</TableCell>
              <TableCell>{t('workflows.created')}</TableCell>
              <TableCell>{t('workflows.published')}</TableCell>
              <TableCell>{t('workflows.definitionHash')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {revisions.map((revision) => (
              <TableRow key={revision.version}>
                <TableCell>v{revision.version}</TableCell>
                <TableCell>
                  <Chip
                    color={
                      revision.status === workflowStatus.published
                        ? 'success'
                        : 'warning'
                    }
                    label={t(`workflows.statuses.${revision.status}`)}
                    size="small"
                  />
                </TableCell>
                <TableCell>
                  <Timestamp
                    fallback={t('workflows.notAvailable')}
                    value={revision.created_at}
                  />
                </TableCell>
                <TableCell>
                  <Timestamp
                    fallback={t('workflows.notAvailable')}
                    value={revision.published_at}
                  />
                </TableCell>
                <TableCell sx={{ fontFamily: monospaceFontFamily }}>
                  {revision.definition_hash}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Box>
    </Paper>
  );
};
