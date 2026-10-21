import { Box, Button, IconButton, Tooltip } from '@mui/material';
import { ChevronDownIcon, ChevronRightIcon, CopyIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { codeTypography } from '../app/theme';
import { copyToClipboard } from './clipboard';
import { captionIconSize, compactIconSize } from './iconSizes';
import {
  formatJson,
  isJsonContainer,
  jsonBrackets,
  jsonEntries,
  jsonEllipsis,
  jsonInlineTokens,
  type JsonContainer,
  type JsonToken,
  type JsonTokenKind,
} from './jsonTokens';
import { useToast } from './useToast';

/** Characters rendered on a line that does not wrap, such as a table cell. */
const noWrapLength = 200;
/** Characters of a collapsed member's one-line preview. */
const collapsedPreviewLength = 80;
/** Members shown before a large object or array asks to show the rest. */
const initialMemberLimit = 50;
/** Nesting levels the structured view opens by default. */
const defaultOpenDepth = 2;
/** Indent of each nesting level, in theme spacing units. */
const levelIndent = 4;

// Keys and punctuation step back so the values read first; color stays
// within the text tokens, since the semantic colors carry status meanings.
const tokenColor: Record<JsonTokenKind, string> = {
  key: 'text.secondary',
  punctuation: 'text.secondary',
  string: 'text.primary',
  literal: 'text.primary',
};

const Tokens = ({ tokens }: { tokens: JsonToken[] }) =>
  tokens.map((token, index) => (
    <Box component="span" key={index} sx={{ color: tokenColor[token.kind] }}>
      {token.text}
    </Box>
  ));

/**
 * A JSON value on one line, for table cells, history entries and other text
 * runs. `maxLength` bounds the rendered characters; `noWrap` keeps it to one
 * line with an ellipsis instead of wrapping, and bounds it by default.
 */
export const JsonInline = ({
  value,
  noWrap = false,
  maxLength = noWrap ? noWrapLength : undefined,
}: {
  value: unknown;
  maxLength?: number;
  noWrap?: boolean;
}) => (
  <Box
    component="code"
    sx={[
      codeTypography,
      noWrap
        ? {
            display: 'block',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }
        : { overflowWrap: 'anywhere', whiteSpace: 'pre-wrap' },
    ]}
  >
    <Tokens tokens={jsonInlineTokens(value, maxLength)} />
  </Box>
);

const CopyJsonButton = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const copy = async () => {
    try {
      await copyToClipboard(formatJson(value));
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
  return (
    <Tooltip title={t('json.copy')}>
      <IconButton
        aria-label={t('json.copy')}
        onClick={(event) => {
          event.stopPropagation();
          void copy();
        }}
        sx={{ position: 'absolute', right: 4, top: 4 }}
      >
        <CopyIcon size={compactIconSize} />
      </IconButton>
    </Tooltip>
  );
};

const Punctuation = ({ children }: { children: ReactNode }) => (
  <Box component="span" sx={{ color: 'text.secondary' }}>
    {children}
  </Box>
);

const Key = ({ name, array }: { name: string; array: boolean }) => (
  <Box component="span" sx={{ color: 'text.secondary' }}>
    {array ? name : JSON.stringify(name)}
    {': '}
  </Box>
);

/** Lines up rows whose value has no disclosure button with those that do. */
const toggleWidth = 20;

const MemberRow = ({
  name,
  array,
  value,
  depth,
  last,
}: {
  name: string;
  array: boolean;
  value: unknown;
  depth: number;
  last: boolean;
}) => {
  const comma = last ? null : <Punctuation>,</Punctuation>;
  if (isJsonContainer(value) && jsonEntries(value).length)
    return (
      <JsonNode
        depth={depth}
        label={<Key array={array} name={name} />}
        name={name}
        suffix={comma}
        value={value}
      />
    );
  return (
    <Box sx={{ pl: `${toggleWidth}px` }}>
      <Key array={array} name={name} />
      <Tokens tokens={jsonInlineTokens(value)} />
      {comma}
    </Box>
  );
};

/** An object or array whose members can be shown or hidden. */
const JsonNode = ({
  value,
  depth,
  name,
  label,
  suffix,
}: {
  value: JsonContainer;
  depth: number;
  name?: string;
  label?: ReactNode;
  suffix?: ReactNode;
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(depth < defaultOpenDepth);
  const [showAll, setShowAll] = useState(false);
  const entries = jsonEntries(value);
  const shown = showAll ? entries : entries.slice(0, initialMemberLimit);
  const [openBracket, closeBracket] = jsonBrackets(value);
  const array = Array.isArray(value);
  const preview = jsonInlineTokens(value, collapsedPreviewLength);
  const toggleLabel = name
    ? t(open ? 'json.collapseNamed' : 'json.expandNamed', { name })
    : t(open ? 'json.collapse' : 'json.expand');
  const ToggleIcon = open ? ChevronDownIcon : ChevronRightIcon;
  return (
    <Box>
      <Box sx={{ alignItems: 'flex-start', display: 'flex' }}>
        <IconButton
          aria-expanded={open}
          aria-label={toggleLabel}
          onClick={() => setOpen(!open)}
          sx={{
            borderRadius: 0.5,
            flexShrink: 0,
            height: toggleWidth,
            p: 0,
            width: toggleWidth,
          }}
        >
          <ToggleIcon size={captionIconSize} />
        </IconButton>
        <Box sx={{ minWidth: 0 }}>
          {label}
          {open ? (
            <Punctuation>{openBracket}</Punctuation>
          ) : (
            <>
              <Tokens tokens={preview} />
              {preview.at(-1) === jsonEllipsis && (
                <Punctuation>
                  {' '}
                  {t(array ? 'json.items' : 'json.keys', {
                    count: entries.length,
                  })}
                </Punctuation>
              )}
              {suffix}
            </>
          )}
        </Box>
      </Box>
      {open && (
        <>
          <Box
            sx={{
              borderLeft: 1,
              // After the shorthand, which would reset it to the text color.
              borderLeftColor: 'divider',
              ml: `${toggleWidth / 2 - 0.5}px`,
              pl: levelIndent - 1,
            }}
          >
            {shown.map(([key, child], index) => (
              <MemberRow
                array={array}
                depth={depth + 1}
                key={key}
                last={index === entries.length - 1}
                name={key}
                value={child}
              />
            ))}
            {shown.length < entries.length && (
              <Button
                onClick={() => setShowAll(true)}
                sx={{ minHeight: 0, ml: `${toggleWidth}px`, py: 0 }}
              >
                {t('json.showAll', { count: entries.length })}
              </Button>
            )}
          </Box>
          <Box sx={{ pl: `${toggleWidth}px` }}>
            <Punctuation>{closeBracket}</Punctuation>
            {suffix}
          </Box>
        </>
      )}
    </Box>
  );
};

/**
 * A JSON value laid out by its structure, for detail panels: nested objects
 * and arrays can be collapsed, and the whole value copied. A scalar stays on
 * one line.
 */
export const JsonBlock = ({ value }: { value: unknown }) => {
  if (!isJsonContainer(value) || !jsonEntries(value).length)
    return <JsonInline value={value} />;
  return (
    <Box
      sx={[
        codeTypography,
        {
          bgcolor: 'background.inset',
          border: 1,
          borderColor: 'divider',
          borderRadius: 1,
          overflowWrap: 'anywhere',
          position: 'relative',
          // Room for the copy button beside the first line.
          pl: 1,
          pr: 9,
          py: 1.5,
          whiteSpace: 'pre-wrap',
        },
      ]}
    >
      <CopyJsonButton value={value} />
      <JsonNode depth={0} value={value} />
    </Box>
  );
};
