import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PlusIcon, Trash2Icon, Undo2Icon } from 'lucide-react';
import { tagColorCodes, type TagColor } from '../../app/tagColors';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { codeTypography } from '../../app/theme';
import { utf8ByteLength } from '../extensions/utf8';
import { MAX_SYSTEM_TAG_BYTES } from '../records/constants';
import { systemTagLabel } from '../records/systemTags';
import { updateTagColors, type TagColors } from '../tag-colors/api';
import { MAX_TAG_COLORS } from '../tag-colors/constants';
import { tagColorsOptions } from '../tag-colors/queryOptions';
import { tagColorQueryKeys } from '../tag-colors/queryKeys';
import { TagColorDot } from '../tag-colors/TagColorDot';
import { tagChipSx, tagColorOf } from '../tag-colors/tagColors';
import { tagColorSelectMinWidth } from './constants';

const ColorSelect = ({
  disabled,
  label,
  onChange,
  value,
}: {
  disabled: boolean;
  label: string;
  onChange: (color: TagColor) => void;
  value: TagColor;
}) => {
  const { t } = useTranslation();
  return (
    <TextField
      disabled={disabled}
      label={label}
      onChange={(event) => onChange(event.target.value as TagColor)}
      select
      slotProps={{
        select: {
          renderValue: (color) => (
            <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
              <TagColorDot color={color as TagColor} />
              {t(`workspace.tagColorNames.${color as TagColor}`)}
            </Box>
          ),
        },
      }}
      sx={{ minWidth: tagColorSelectMinWidth }}
      value={value}
    >
      {tagColorCodes.map((color) => (
        <MenuItem key={color} sx={{ gap: 1 }} value={color}>
          <TagColorDot color={color} />
          {t(`workspace.tagColorNames.${color}`)}
        </MenuItem>
      ))}
    </TextField>
  );
};

/** The tags with a color: Core tags first, then the workspace's own. */
const coloredTags = (colors: TagColors) => {
  const defaults = Object.keys(colors.defaults).sort();
  const own = Object.keys(colors.overrides)
    .filter((tag) => !Object.hasOwn(colors.defaults, tag))
    .sort();
  return [...defaults, ...own];
};

const TagColorsEditor = ({ colors }: { colors: TagColors }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [newTag, setNewTag] = useState('');
  const [newColor, setNewColor] = useState<TagColor>(tagColorCodes[0]);
  const save = useMutation({
    meta: { toast: false },
    mutationFn: updateTagColors,
    onSuccess: (saved) =>
      queryClient.setQueryData(tagColorQueryKeys.all(), saved),
  });
  const tags = coloredTags(colors);
  const trimmedTag = newTag.trim();
  // The server bounds a tag in UTF-8 bytes, which `maxLength` does not count.
  const tagTooLong = utf8ByteLength(trimmedTag) > MAX_SYSTEM_TAG_BYTES;
  const canAdd = trimmedTag !== '' && !tagTooLong;
  const full = Object.keys(colors.overrides).length >= MAX_TAG_COLORS;
  const add = () => {
    save.mutate(
      { set: { [trimmedTag]: newColor } },
      { onSuccess: () => setNewTag('') },
    );
  };

  return (
    <Stack spacing={2}>
      {tags.map((tag) => {
        const color = tagColorOf(colors, tag);
        if (!color) return null;
        const label = systemTagLabel(tag);
        const isDefault = Object.hasOwn(colors.defaults, tag);
        const overridden = Object.hasOwn(colors.overrides, tag);
        return (
          <Paper key={tag} sx={{ p: 2 }}>
            <Stack
              direction={{ xs: 'column', md: 'row' }}
              spacing={1.5}
              sx={{ alignItems: { md: 'center' } }}
            >
              <Box sx={{ flex: 1, minWidth: 0 }}>
                <Chip
                  label={label}
                  sx={[{ maxWidth: '100%' }, tagChipSx(color)]}
                  title={label}
                  variant="outlined"
                />
                <Typography
                  color="text.secondary"
                  sx={{ ...codeTypography, mt: 0.5, wordBreak: 'break-all' }}
                >
                  {tag}
                  {isDefault &&
                    !overridden &&
                    ` · ${t('workspace.tagColorDefault')}`}
                </Typography>
              </Box>
              <ColorSelect
                disabled={save.isPending}
                label={t('workspace.tagColorFor', { tag: label })}
                onChange={(next) => save.mutate({ set: { [tag]: next } })}
                value={color}
              />
              {isDefault ? (
                <Button
                  aria-label={t('workspace.resetTagColorFor', { tag: label })}
                  disabled={save.isPending || !overridden}
                  onClick={() => save.mutate({ reset: [tag] })}
                  startIcon={<Undo2Icon />}
                >
                  {t('workspace.resetTagColor')}
                </Button>
              ) : (
                <Button
                  aria-label={t('workspace.removeTagColorFor', { tag: label })}
                  disabled={save.isPending}
                  onClick={() => save.mutate({ reset: [tag] })}
                  startIcon={<Trash2Icon />}
                >
                  {t('workspace.remove')}
                </Button>
              )}
            </Stack>
          </Paper>
        );
      })}
      <Paper
        component="form"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          if (canAdd) add();
        }}
        sx={{ p: 2 }}
      >
        <Stack
          direction={{ xs: 'column', md: 'row' }}
          spacing={1.5}
          sx={{ alignItems: { md: 'flex-start' } }}
        >
          <TextField
            disabled={save.isPending || full}
            error={tagTooLong}
            helperText={
              full
                ? t('workspace.tagColorsFull', { max: MAX_TAG_COLORS })
                : tagTooLong
                  ? t('workspace.newTagColorTooLong', {
                      max: MAX_SYSTEM_TAG_BYTES,
                    })
                  : t('workspace.newTagColorHelp')
            }
            label={t('workspace.newTagColorTag')}
            onChange={(event) => setNewTag(event.target.value)}
            slotProps={{ htmlInput: { maxLength: MAX_SYSTEM_TAG_BYTES } }}
            sx={{ flex: 1 }}
            value={newTag}
          />
          <ColorSelect
            disabled={save.isPending || full}
            label={t('workspace.newTagColorColor')}
            onChange={setNewColor}
            value={newColor}
          />
          <Button
            disabled={save.isPending || full || !canAdd}
            startIcon={<PlusIcon />}
            sx={{ mt: { md: 0.25 } }}
            type="submit"
          >
            {t('workspace.addTagColor')}
          </Button>
        </Stack>
      </Paper>
      <ApiErrorAlert error={save.error} />
    </Stack>
  );
};

export const WorkspaceTagColorsSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const colors = useQuery({ ...tagColorsOptions(), enabled: canManage });
  if (!canManage)
    return (
      <Alert severity="error">{t('workspace.notAuthorizedTagColors')}</Alert>
    );
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <Typography component="h2" variant="h6">
        {t('workspace.tagColorsTitle')}
      </Typography>
      <Typography color="text.secondary">
        {t('workspace.tagColorsDescription')}
      </Typography>
      {colors.isPending && (
        <CircularProgress aria-label={t('workspace.loadingTagColors')} />
      )}
      <ApiErrorAlert error={colors.error} />
      {colors.data && <TagColorsEditor colors={colors.data} />}
    </Stack>
  );
};
