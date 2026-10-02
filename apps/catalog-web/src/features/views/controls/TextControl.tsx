import { Link, TextField, Typography } from '@mui/material';
import type { ComponentProps } from 'react';
import { attributeLabel } from '../../entities/entityDisplay';
import type { ValueEditorProps } from '../components/componentTypes';
import { EachValue } from './EachValue';

/** Single-line string editor for controls that only change the input mode. */
export const TextControlEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
  type,
  validate,
  defaultHelperText,
  leftToRight = false,
}: ValueEditorProps & {
  type: 'email' | 'tel' | 'url';
  /** Shows the form's validation message while typing, before Save. */
  validate?: (value: string) => string | undefined;
  defaultHelperText?: string;
  leftToRight?: boolean;
}) => {
  const message = error ?? validate?.(value);
  return (
    <TextField
      fullWidth
      type={type}
      label={attributeLabel(attribute)}
      value={value}
      disabled={disabled}
      required={required}
      error={Boolean(message)}
      helperText={message ?? helperText ?? defaultHelperText}
      onChange={(event) => {
        if (!disabled) onChange(event.target.value);
      }}
      slotProps={{
        htmlInput: {
          inputMode: type,
          autoCapitalize: 'none',
          spellCheck: false,
          dir: leftToRight ? 'ltr' : undefined,
        },
      }}
    />
  );
};

/**
 * Stored text that becomes a link when `href` accepts it. Invalid values stay
 * readable as text. Multi-valued projections render one value per line.
 */
export const LinkedTextValue = ({
  value,
  href,
  linkProps,
  leftToRight = false,
}: {
  value: unknown;
  href: (value: unknown) => string | undefined;
  linkProps?: (text: string) => ComponentProps<typeof Link>;
  leftToRight?: boolean;
}) => (
  <EachValue
    value={value}
    renderItem={(item) => {
      const text =
        typeof item === 'object' ? JSON.stringify(item) : String(item);
      const target = href(item);
      // Keep left-to-right values such as phone numbers intact in RTL layouts.
      const dir = leftToRight ? 'ltr' : undefined;
      const sx = {
        overflowWrap: 'anywhere',
        unicodeBidi: leftToRight ? 'isolate' : undefined,
      } as const;
      return target ? (
        <Link
          href={target}
          dir={dir}
          variant="body2"
          sx={sx}
          // Table rows and cards are clickable; following a link must not activate them.
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
          {...linkProps?.(text)}
        >
          {text}
        </Link>
      ) : (
        <Typography dir={dir} variant="body2" sx={sx}>
          {text}
        </Typography>
      );
    }}
  />
);
