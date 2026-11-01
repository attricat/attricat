import {
  Box,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { RevisionStatusChip } from '../../components/RevisionStatusChip';
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
              <TableCell>{t('workflows.revision')}</TableCell>
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
                  <RevisionStatusChip size="small" status={revision.status} />
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Timestamp
                    fallback={t('workflows.notAvailable')}
                    value={revision.created_at}
                  />
                </TableCell>
                <TableCell sx={{ whiteSpace: 'nowrap' }}>
                  <Timestamp
                    fallback={t('workflows.notAvailable')}
                    value={revision.published_at}
                  />
                </TableCell>
                <TableCell
                  sx={{ fontFamily: monoFontFamily, overflowWrap: 'anywhere' }}
                >
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
