import { Dialog, DialogContent, DialogTitle, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { EntityItem } from '../entities/api';

type Props = {
  entity: EntityItem | null;
  onClose: () => void;
};

export const SearchInfoDialog = ({ entity, onClose }: Props) => {
  const { t } = useTranslation();
  const details = entity?.match_explanations
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
    <Dialog onClose={onClose} open={Boolean(entity)}>
      <DialogTitle>{t('explorer.searchInfo')}</DialogTitle>
      <DialogContent>
        <Typography>{details || t('explorer.noSearchDetails')}</Typography>
      </DialogContent>
    </Dialog>
  );
};
