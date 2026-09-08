import { createTheme } from '@mui/material';

export const theme = createTheme({
  palette: { background: { default: '#f7f7f5' }, primary: { main: '#1565c0' } },
  shape: { borderRadius: 4 },
  typography: {
    fontFamily: 'Inter, ui-sans-serif, system-ui, sans-serif',
    body1: { fontSize: '0.875rem', lineHeight: 1.45 },
    body2: { fontSize: '0.8125rem', lineHeight: 1.4 },
    h2: { fontSize: '2.25rem', lineHeight: 1.1 },
    h3: { fontSize: '1.75rem', lineHeight: 1.15 },
    h4: { fontSize: '1.625rem', lineHeight: 1.25 },
    h5: { fontSize: '1.25rem', lineHeight: 1.3 },
    h6: { fontSize: '1.0625rem', lineHeight: 1.35 },
    subtitle1: { fontSize: '0.9375rem', lineHeight: 1.4 },
    subtitle2: { fontSize: '0.8125rem', lineHeight: 1.35 },
  },
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        'a:not(.MuiButtonBase-root), a:not(.MuiButtonBase-root):visited': {
          color: '#1565c0',
        },
      },
    },
    MuiButton: { defaultProps: { size: 'small' } },
    MuiTextField: { defaultProps: { size: 'small' } },
    MuiFormControl: { defaultProps: { size: 'small' } },
  },
});
