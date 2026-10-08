import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  ReusableAttribute,
  ReusableAttributeGroup,
} from '../../reusable-attributes/api';
import {
  REUSABLE_ATTRIBUTE_NAMESPACE_SEPARATOR,
  reusableSelectionTypes,
  type ReusableSelectionType,
} from '../constants';
import { lexiconText } from '../../lexicon/lexicon';
import { ValueTypeIcon } from './ValueTypeLabel';

type Props = {
  attachAttributeDisabled: boolean;
  attachGroupDisabled: boolean;
  attributes: readonly ReusableAttribute[];
  groups: readonly ReusableAttributeGroup[];
  onAttachAttribute: (revisionId: string) => void;
  onAttachGroup: (groupId: string) => void;
  onClose: () => void;
  open: boolean;
};

/**
 * Chooses a reusable attribute or attribute group to attach to an entity.
 * Mount a fresh instance for each opening so selections start empty.
 */
export const ReusableAttributeAttachDialog = ({
  attachAttributeDisabled,
  attachGroupDisabled,
  attributes,
  groups,
  onAttachAttribute,
  onAttachGroup,
  onClose,
  open,
}: Props) => {
  const { t } = useTranslation();
  const [selectionType, setSelectionType] =
    useState<ReusableSelectionType | null>(null);
  const [selectedAttribute, setSelectedAttribute] = useState('');
  const [selectedGroup, setSelectedGroup] = useState('');

  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open={open}>
      <DialogTitle>{t('entities.addReusableAttributeOrGroup')}</DialogTitle>
      <DialogContent>
        {selectionType === null && (
          <Stack spacing={2} sx={{ pt: 1 }}>
            <Typography>
              {t('entities.chooseReusableAttributeOrGroup')}
            </Typography>
            <Stack direction={{ sm: 'row' }} spacing={1}>
              <Button
                onClick={() =>
                  setSelectionType(reusableSelectionTypes.attribute)
                }
                variant="outlined"
              >
                {t('entities.additionalAttributes')}
              </Button>
              <Button
                onClick={() => setSelectionType(reusableSelectionTypes.group)}
                variant="outlined"
              >
                {t('entities.attributeGroup')}
              </Button>
            </Stack>
          </Stack>
        )}
        {selectionType === reusableSelectionTypes.attribute && (
          <TextField
            fullWidth
            label={t('entities.additionalAttributes')}
            onChange={(event) => setSelectedAttribute(event.target.value)}
            select
            sx={{ mt: 1 }}
            value={selectedAttribute}
          >
            <MenuItem value="">
              {t('entities.selectAdditionalAttribute')}
            </MenuItem>
            {attributes.map((attribute) => (
              <MenuItem key={attribute.id} value={attribute.id}>
                {/* Also rendered as the selected value, so keep the layout here. */}
                <Box
                  component="span"
                  sx={{
                    alignItems: 'center',
                    display: 'inline-flex',
                    gap: 3,
                  }}
                >
                  <ValueTypeIcon valueType={attribute.value_type} />
                  {t('entities.reusableAttributeOption', {
                    qualifiedCode: `${attribute.namespace}${REUSABLE_ATTRIBUTE_NAMESPACE_SEPARATOR}${attribute.code}`,
                    name: lexiconText(attribute.name),
                    version: attribute.version,
                  })}
                </Box>
              </MenuItem>
            ))}
          </TextField>
        )}
        {selectionType === reusableSelectionTypes.group && (
          <TextField
            fullWidth
            label={t('entities.attributeGroup')}
            onChange={(event) => setSelectedGroup(event.target.value)}
            select
            sx={{ mt: 1 }}
            value={selectedGroup}
          >
            <MenuItem value="">{t('entities.selectAttributeGroup')}</MenuItem>
            {groups.map((group) => (
              <MenuItem key={group.id} value={group.id}>
                {group.name}
              </MenuItem>
            ))}
          </TextField>
        )}
      </DialogContent>
      <DialogActions>
        {selectionType !== null && (
          <Button onClick={() => setSelectionType(null)}>
            {t('entities.back')}
          </Button>
        )}
        <Button onClick={onClose}>{t('common.cancel')}</Button>
        {selectionType === reusableSelectionTypes.attribute && (
          <Button
            disabled={!selectedAttribute || attachAttributeDisabled}
            onClick={() => onAttachAttribute(selectedAttribute)}
            variant="contained"
          >
            {t('entities.addAttribute')}
          </Button>
        )}
        {selectionType === reusableSelectionTypes.group && (
          <Button
            disabled={!selectedGroup || attachGroupDisabled}
            onClick={() => onAttachGroup(selectedGroup)}
            variant="contained"
          >
            {t('entities.addAttributeGroup')}
          </Button>
        )}
      </DialogActions>
    </Dialog>
  );
};
