import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Stack } from '@mui/material';
import { ImageUpIcon, Trash2Icon } from 'lucide-react';
import { type ChangeEvent, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { removeAvatar, uploadAvatar } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  avatarMaxBytes,
  avatarMaxMegabytes,
  avatarMimeTypes,
} from './constants';

/** Uploads, replaces, and removes the caller's avatar in this workspace. */
export const AvatarControls = ({
  disabled,
  hasAvatar,
  onProgress,
}: {
  disabled: boolean;
  hasAvatar: boolean;
  /** Upload percentage, or `null` when no upload is in flight. */
  onProgress: (progress: number | null) => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const inputRef = useRef<HTMLInputElement>(null);
  const [rejection, setRejection] = useState<string | null>(null);
  const refreshSession = () =>
    client.invalidateQueries({ queryKey: authQueryKeys.session() });
  const upload = useMutation({
    mutationFn: (file: File) => uploadAvatar(file, onProgress),
    onSettled: () => {
      onProgress(null);
      return refreshSession();
    },
  });
  const remove = useMutation({
    mutationFn: removeAvatar,
    onSettled: refreshSession,
  });
  const pending = upload.isPending || remove.isPending;
  const chooseFile = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    // Reset so choosing the same file again still triggers a change.
    event.target.value = '';
    if (!file) return;
    upload.reset();
    remove.reset();
    if (!avatarMimeTypes.includes(file.type)) {
      setRejection(t('profile.avatarWrongType'));
      return;
    }
    if (file.size > avatarMaxBytes) {
      setRejection(t('profile.avatarTooLarge', { max: avatarMaxMegabytes }));
      return;
    }
    setRejection(null);
    onProgress(0);
    upload.mutate(file);
  };
  const error = rejection ?? (upload.error ?? remove.error)?.message;

  return (
    <Stack spacing={2} sx={{ alignItems: 'flex-start' }}>
      <Stack direction="row" spacing={2} sx={{ flexWrap: 'wrap' }} useFlexGap>
        <Button
          disabled={disabled || pending}
          onClick={() => inputRef.current?.click()}
          startIcon={<ImageUpIcon />}
          variant="outlined"
        >
          {hasAvatar ? t('profile.changeAvatar') : t('profile.uploadAvatar')}
        </Button>
        {hasAvatar && (
          <Button
            color="error"
            disabled={disabled || pending}
            onClick={() => {
              setRejection(null);
              upload.reset();
              remove.mutate();
            }}
            startIcon={<Trash2Icon />}
          >
            {t('profile.removeAvatar')}
          </Button>
        )}
      </Stack>
      <input
        accept={avatarMimeTypes.join(',')}
        aria-label={t('profile.avatarFile')}
        hidden
        onChange={chooseFile}
        ref={inputRef}
        type="file"
      />
      {error && <Alert severity="error">{error}</Alert>}
    </Stack>
  );
};
