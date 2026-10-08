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
import { Link } from '@tanstack/react-router';
import { PencilIcon, SendIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import type { ReusableAttribute } from './api';
import {
  ATTRIBUTES_TABLE_COLUMN_COUNT,
  STATUS_DRAFT,
  STATUS_PUBLISHED,
} from './constants';
import { lexiconText } from '../lexicon/lexicon';
import { ValueTypeLabel } from '../entities/components/ValueTypeLabel';
import { EmptyState } from '../../components/EmptyState';
import { ReusableAttributeIcon } from '../../components/systemIcons';

export const ReusableAttributesTable = ({
  attributes,
  onPublish,
  publishing,
}: {
  attributes: ReusableAttribute[];
  onPublish: (revisionId: string) => void;
  publishing: boolean;
}) => {
  const { t } = useTranslation();
  return (
    <Paper sx={{ mt: 3 }}>
      <Box sx={{ overflowX: 'auto' }}>
        <Table>
          <TableHead>
            <TableRow>
              <TableCell>{t('reusableAttributes.columns.attribute')}</TableCell>
              <TableCell>{t('reusableAttributes.columns.version')}</TableCell>
              <TableCell>{t('reusableAttributes.columns.type')}</TableCell>
              <TableCell>{t('reusableAttributes.columns.status')}</TableCell>
              <TableCell align="right">
                {t('reusableAttributes.columns.actions')}
              </TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {attributes.map((attribute) => (
              <TableRow hover key={attribute.definition_id}>
                <TableCell>
                  <Link
                    params={{ definitionId: attribute.definition_id }}
                    to="/manage/reusable-attributes/$definitionId"
                  >
                    <Typography>{lexiconText(attribute.name)}</Typography>
                    <Typography color="text.secondary" variant="caption">
                      {attribute.namespace}:{attribute.code}
                    </Typography>
                  </Link>
                </TableCell>
                <TableCell>v{attribute.version}</TableCell>
                <TableCell>
                  <ValueTypeLabel valueType={attribute.value_type} />
                </TableCell>
                <TableCell>
                  <Chip
                    color={
                      attribute.status === STATUS_PUBLISHED
                        ? 'success'
                        : 'warning'
                    }
                    label={attribute.status}
                    size="small"
                  />
                </TableCell>
                <TableCell align="right">
                  <RouterButton
                    params={{ definitionId: attribute.definition_id }}
                    size="small"
                    startIcon={<PencilIcon />}
                    to="/manage/reusable-attributes/$definitionId"
                  >
                    {t('reusableAttributes.edit')}
                  </RouterButton>
                  {attribute.status === STATUS_DRAFT && (
                    <Button
                      disabled={publishing}
                      onClick={() => onPublish(attribute.id)}
                      size="small"
                      startIcon={<SendIcon />}
                    >
                      {t('reusableAttributes.publish')}
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
            {!attributes.length && (
              <TableRow>
                <TableCell colSpan={ATTRIBUTES_TABLE_COLUMN_COUNT}>
                  <EmptyState
                    icon={ReusableAttributeIcon}
                    title={t('reusableAttributes.empty')}
                  />
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </Box>
    </Paper>
  );
};
