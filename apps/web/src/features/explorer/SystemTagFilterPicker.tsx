import {
  Autocomplete,
  Box,
  Button,
  Popover,
  TextField,
  Typography,
  type PopperProps,
} from '@mui/material';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useRef, useState, type ReactNode, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { errorMessage } from '../../api/errors';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import { smallIconSize } from '../../components/iconSizes';
import { SystemTagIcon } from '../../components/systemIcons';
import { useDebouncedValue } from '../../components/useDebouncedValue';
import { getSystemTagFacet, type SystemTagCount } from '../records/api';
import { SYSTEM_TAG_FACET_LIMIT } from '../records/constants';
import { recordQueryKeys } from '../records/queryKeys';
import {
  systemTagFacetStaleTimeMs,
  systemTagPickerWidth,
  systemTagSearchDebounceMs,
} from './constants';

type Props = {
  blueprint: string;
  /** Tags already filtered by; they are left out of the list. */
  selected: string[];
  disabled?: boolean;
  buttonRef?: Ref<HTMLButtonElement>;
  onAdd: (tag: string) => void;
};

/**
 * Lists the options below the input inside the popover, not in a portal.
 * The popper class is left out: it positions the list over the page.
 */
const InlinePopper = ({ children, id, role, style }: PopperProps) => (
  <div id={id} role={role} style={style}>
    {children as ReactNode}
  </div>
);

/**
 * Adds a system tag filter: a button that opens a searchable list of the
 * tags the blueprint's records carry, most used first. The list is requested
 * only while the picker is open.
 */
export const SystemTagFilterPicker = ({
  blueprint,
  selected,
  disabled = false,
  buttonRef,
  onAdd,
}: Props) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [input, setInput] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const prefix = useDebouncedValue(input.trim(), systemTagSearchDebounceMs);
  const request = {
    blueprint: { code: blueprint },
    ...(prefix ? { prefix } : {}),
    limit: SYSTEM_TAG_FACET_LIMIT,
  };
  const facet = useQuery({
    queryKey: recordQueryKeys.systemTagFacet(request),
    queryFn: ({ signal }) => getSystemTagFacet(request, signal),
    enabled: anchor !== null,
    staleTime: systemTagFacetStaleTimeMs,
    placeholderData: keepPreviousData,
  });
  const options = (facet.data?.items ?? []).filter(
    (item) => !selected.includes(item.tag),
  );
  const close = () => {
    setAnchor(null);
    setInput('');
  };
  const emptyText = facet.isError
    ? errorMessage(facet.error)
    : facet.isPending
      ? t('explorer.loadingSystemTags')
      : prefix
        ? t('explorer.noMatchingSystemTags')
        : t('explorer.noSystemTags');

  return (
    <>
      <Button
        color="primary"
        disabled={disabled}
        onClick={(event) => setAnchor(event.currentTarget)}
        ref={buttonRef}
        size="small"
        startIcon={<SystemTagIcon size={smallIconSize} />}
        sx={compactOutlinedActionButtonSx}
        variant="outlined"
      >
        {t('explorer.addSystemTagFilter')}
      </Button>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        onClose={close}
        open={anchor !== null}
        slotProps={{
          transition: { onEntered: () => inputRef.current?.focus() },
        }}
      >
        <Box sx={{ p: 1.5, width: systemTagPickerWidth }}>
          <Autocomplete<SystemTagCount>
            disablePortal
            filterOptions={(items) => items}
            getOptionLabel={(item) => item.tag}
            inputValue={input}
            isOptionEqualToValue={(item, value) => item.tag === value.tag}
            loading={facet.isFetching}
            noOptionsText={emptyText}
            onChange={(_, item) => {
              if (!item) return;
              onAdd(item.tag);
              close();
            }}
            onClose={(_, reason) => {
              if (reason === 'escape') close();
            }}
            onInputChange={(_, value, reason) => {
              if (reason === 'input' || reason === 'clear') setInput(value);
            }}
            open
            options={options}
            renderInput={(params) => (
              <TextField
                {...params}
                inputRef={inputRef}
                label={t('explorer.systemTag')}
                placeholder={t('explorer.searchSystemTags')}
              />
            )}
            renderOption={({ key, ...props }, item) => (
              <li key={key} {...props}>
                <Box
                  sx={{
                    alignItems: 'baseline',
                    display: 'flex',
                    gap: 1,
                    justifyContent: 'space-between',
                    width: '100%',
                  }}
                >
                  <Typography sx={{ wordBreak: 'break-word' }} variant="body2">
                    {item.tag}
                  </Typography>
                  <Typography color="text.secondary" variant="caption">
                    {t('explorer.systemTagRecordCount', { count: item.count })}
                  </Typography>
                </Box>
              </li>
            )}
            size="small"
            slots={{ popper: InlinePopper }}
            value={null}
          />
        </Box>
      </Popover>
    </>
  );
};
