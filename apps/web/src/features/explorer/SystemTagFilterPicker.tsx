import {
  Autocomplete,
  Box,
  Button,
  Popover,
  TextField,
  Typography,
  type PopperProps,
} from '@mui/material';
import {
  keepPreviousData,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { useRef, useState, type ReactNode, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { errorMessage } from '../../api/errors';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import { smallIconSize } from '../../components/iconSizes';
import { SystemTagIcon } from '../../components/systemIcons';
import { useDebouncedValue } from '../../components/useDebouncedValue';
import {
  getSystemTagFacet,
  type SystemTagCount,
  type SystemTagFacetResponse,
} from '../records/api';
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

/** Whether `tag` starts with `prefix`, ignoring case as the server does. */
const startsWithIgnoringCase = (tag: string, prefix: string) =>
  tag.toLowerCase().startsWith(prefix.toLowerCase());

/**
 * Adds a system tag filter: a button that opens a searchable list of the
 * tags the blueprint's records carry, most used first. The list is requested
 * only while the picker is open. Each request reads every record of the
 * blueprint, so typing filters the first list in the browser when it holds
 * every tag, and asks the server for a prefix only when it was truncated.
 * A fresh, complete answer for a shorter prefix also covers longer ones.
 */
export const SystemTagFilterPicker = ({
  blueprint,
  selected,
  disabled = false,
  buttonRef,
  onAdd,
}: Props) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [input, setInput] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const typed = input.trim();
  const debouncedPrefix = useDebouncedValue(typed, systemTagSearchDebounceMs);
  const facetRequest = (prefix?: string) => ({
    blueprint: { code: blueprint },
    ...(prefix ? { prefix } : {}),
    limit: SYSTEM_TAG_FACET_LIMIT,
  });
  const allRequest = facetRequest();
  const all = useQuery({
    queryKey: recordQueryKeys.systemTagFacet(allRequest),
    queryFn: ({ signal }) => getSystemTagFacet(allRequest, signal),
    enabled: anchor !== null,
    staleTime: systemTagFacetStaleTimeMs,
  });
  // A prefix only reaches the server when the first list left tags out.
  const searchesServer = all.data?.truncated === true && typed !== '';
  // The shortest typed prefix whose fresh answer listed every match.
  const completePrefix = searchesServer
    ? Array.from({ length: typed.length }, (_, end) =>
        typed.slice(0, end + 1),
      ).find((prefix) => {
        const cached = client.getQueryCache().find<SystemTagFacetResponse>({
          exact: true,
          queryKey: recordQueryKeys.systemTagFacet(facetRequest(prefix)),
        });
        return (
          cached?.state.data?.truncated === false &&
          !cached.isStaleByTime(systemTagFacetStaleTimeMs)
        );
      })
    : undefined;
  const prefixRequest = facetRequest(completePrefix ?? debouncedPrefix);
  const matching = useQuery({
    queryKey: recordQueryKeys.systemTagFacet(prefixRequest),
    queryFn: ({ signal }) => getSystemTagFacet(prefixRequest, signal),
    enabled:
      anchor !== null && searchesServer && prefixRequest.prefix !== undefined,
    staleTime: systemTagFacetStaleTimeMs,
    placeholderData: keepPreviousData,
  });
  // Until the server answers, and for an answer to an earlier prefix, the
  // typed prefix still narrows what is shown.
  const options = (
    (searchesServer ? matching.data?.items : undefined) ??
    all.data?.items ??
    []
  ).filter(
    (item) =>
      startsWithIgnoringCase(item.tag, typed) && !selected.includes(item.tag),
  );
  const error = all.error ?? (searchesServer ? matching.error : null);
  const close = () => {
    setAnchor(null);
    setInput('');
  };
  const emptyText = error
    ? errorMessage(error)
    : all.isPending || (searchesServer && matching.isFetching)
      ? t('explorer.loadingSystemTags')
      : typed
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
      {/* The input takes focus as the popover starts opening, so nothing
          typed right after the click is lost to the opening transition. */}
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        disableAutoFocus
        onClose={close}
        open={anchor !== null}
        slotProps={{
          transition: { onEnter: () => inputRef.current?.focus() },
        }}
      >
        <Box sx={{ p: 1.5, width: systemTagPickerWidth }}>
          <Autocomplete<SystemTagCount>
            disablePortal
            filterOptions={(items) => items}
            getOptionLabel={(item) => item.tag}
            inputValue={input}
            isOptionEqualToValue={(item, value) => item.tag === value.tag}
            loading={all.isFetching || (searchesServer && matching.isFetching)}
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
