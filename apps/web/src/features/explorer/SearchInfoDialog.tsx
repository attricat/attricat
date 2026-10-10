import { Dialog, DialogContent, DialogTitle, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { RecordItem } from '../records/api';

type Props = {
  record: RecordItem | null;
  onClose: () => void;
};

export const SearchInfoDialog = ({ record, onClose }: Props) => {
  const { t } = useTranslation();
  const details = record?.match_explanations
    .map((explanation) =>
      explanation.traversal_depth
        ? t('explorer.matchViaRelationship', {
            count: explanation.traversal_depth,
            term: explanation.term,
          })
        : explanation.matching_attribute_code
          ? t('explorer.matchInAttribute', {
              attribute: explanation.matching_attribute_code,
              term: explanation.term,
            })
          : explanation.term,
    )
    .join(t('explorer.searchDetailsSeparator'));

  return (
    <Dialog onClose={onClose} open={Boolean(record)}>
      <DialogTitle>{t('explorer.searchInfo')}</DialogTitle>
      <DialogContent>
        <Typography>{details || t('explorer.noSearchDetails')}</Typography>
      </DialogContent>
    </Dialog>
  );
};
