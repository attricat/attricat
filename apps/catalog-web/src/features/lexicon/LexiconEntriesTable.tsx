import { useTranslation } from 'react-i18next';
import {
  Box,
  Chip,
  IconButton,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from '@mui/material';
import { PencilIcon, Trash2Icon } from 'lucide-react';
import { LEXICON_SOURCES, LEXICON_MANAGEMENT_NAMESPACES } from './constants';
import { lexiconEntryId, lexiconReferenceId } from './entries';
import { pluralCategoryLabel } from './languages';
import type { StoredLexiconEntry } from './schemas';

export const LexiconEntriesTable = ({
  canEdit,
  entries,
  onDelete,
  onEdit,
  unusedReferences,
}: {
  canEdit: boolean;
  entries: StoredLexiconEntry[];
  onDelete: (entry: StoredLexiconEntry) => void;
  onEdit: (entry: StoredLexiconEntry) => void;
  /** `lexiconReferenceId`s no blueprint or reusable attribute uses. */
  unusedReferences: Set<string>;
}) => {
  const { t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  return (
    <Box sx={{ overflowX: 'auto' }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('lexicon.key')}</TableCell>
            <TableCell>{t('lexicon.context')}</TableCell>
            <TableCell>{t('lexicon.pluralCategory')}</TableCell>
            <TableCell>{t('lexicon.translation')}</TableCell>
            <TableCell>{t('lexicon.source')}</TableCell>
            {canEdit && <TableCell />}
          </TableRow>
        </TableHead>
        <TableBody>
          {entries.map((entry) => (
            <TableRow hover key={lexiconEntryId(entry)}>
              <TableCell>
                <Stack
                  direction="row"
                  spacing={1}
                  sx={{ alignItems: 'center', flexWrap: 'wrap' }}
                >
                  <span>{entry.key}</span>
                  {unusedReferences.has(
                    lexiconReferenceId(entry.key, entry.context),
                  ) && (
                    <Tooltip title={t('lexicon.unusedDescription')}>
                      <Chip
                        label={t('lexicon.unused')}
                        size="small"
                        variant="outlined"
                      />
                    </Tooltip>
                  )}
                </Stack>
              </TableCell>
              <TableCell>
                {entry.context ?? (
                  <Typography color="text.secondary" variant="body2">
                    {t('lexicon.noContext')}
                  </Typography>
                )}
              </TableCell>
              <TableCell>
                <Chip
                  label={pluralCategoryLabel(t, entry.plural_category)}
                  size="small"
                  variant="outlined"
                />
              </TableCell>
              <TableCell sx={{ whiteSpace: 'pre-wrap' }}>
                {entry.text}
              </TableCell>
              <TableCell>
                {entry.source === LEXICON_SOURCES.solutionPack ? (
                  <Stack spacing={0.25}>
                    <span>{t('lexicon.sources.solutionPack')}</span>
                    <Typography color="text.secondary" variant="caption">
                      {entry.solution_pack_id}
                    </Typography>
                  </Stack>
                ) : (
                  t('lexicon.sources.workspace')
                )}
              </TableCell>
              {canEdit && (
                <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                  <Tooltip title={t('lexicon.editTranslation')}>
                    <IconButton
                      aria-label={t('lexicon.editEntry', { key: entry.key })}
                      onClick={() => onEdit(entry)}
                      size="small"
                    >
                      <PencilIcon />
                    </IconButton>
                  </Tooltip>
                  <Tooltip title={t('lexicon.deleteTranslation')}>
                    <IconButton
                      aria-label={t('lexicon.deleteEntry', { key: entry.key })}
                      onClick={() => onDelete(entry)}
                      size="small"
                    >
                      <Trash2Icon />
                    </IconButton>
                  </Tooltip>
                </TableCell>
              )}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  );
};
