import { SvgIcon, type SvgIconProps } from '@mui/material';

// A small connected-catalog mark, kept legible in the compact navigation rail.
export const BrandIcon = (props: SvgIconProps) => (
  <SvgIcon {...props} viewBox="0 0 32 32">
    <path
      d="M9 9.5 16 6l7 3.5v8L16 21l-7-3.5v-8Z"
      fill="none"
      stroke="currentColor"
      strokeLinejoin="round"
      strokeWidth="2.5"
    />
    <path
      d="M9 17.5v5L16 26l7-3.5v-5M16 21v5M9 9.5l7 3.5 7-3.5M16 13v8"
      fill="none"
      stroke="currentColor"
      strokeLinejoin="round"
      strokeWidth="2.5"
    />
  </SvgIcon>
);
