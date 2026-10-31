import {
  Box,
  Paper,
  ScopedCssBaseline,
  Stack,
  ThemeProvider,
  Typography,
} from '@mui/material';
import {
  MUTATION_TYPE_INSERT_CHILD,
  NODE_TYPE_ELEMENT,
  NODE_TYPE_TEXT,
  ROOT_ID,
  type RemoteElementSerialization,
  type RemoteTextSerialization,
} from '@remote-dom/core';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { makeTheme, type DesignMode } from '../../../app/theme';
import { uiKitElements, uiKitVersions } from './generated/uiKit';
import { NativeTree } from './NativeTree';
import { NativeReceiver } from './receiver';

type Localized = { en: string; pl: string };
type Child = RemoteElementSerialization | RemoteTextSerialization;

const designModes: DesignMode[] = ['light', 'dark'];
const latestUiKit = uiKitVersions[uiKitVersions.length - 1];

/** Builds sample trees the way an extension's Worker would send them. */
const sample = (locale: 'en' | 'pl', onEvent: (event: string) => void) => {
  let next = 0;
  const say = (text: Localized): RemoteTextSerialization => ({
    id: String(++next),
    type: NODE_TYPE_TEXT,
    data: text[locale],
  });
  const element = (
    name: keyof typeof uiKitElements,
    properties: Record<string, unknown> = {},
    children: Child[] = [],
    events: string[] = [],
    attributes: Record<string, string> = {},
  ): RemoteElementSerialization => ({
    id: String(++next),
    type: NODE_TYPE_ELEMENT,
    element: uiKitElements[name].tag,
    properties,
    attributes,
    eventListeners: Object.fromEntries(
      events.map((event) => [
        event,
        (payload: unknown) =>
          onEvent(`${name}.${event} ${JSON.stringify(payload)}`),
      ]),
    ),
    children,
  });
  const t = (en: string, pl: string) => ({ en, pl })[locale];
  const options = [
    { value: 'net', label: t('Net price', 'Cena netto') },
    { value: 'gross', label: t('Gross price', 'Cena brutto') },
  ];
  return {
    Stack: element('Stack', { direction: 'horizontal', gap: 'sm' }, [
      element('Badge', {}, [say({ en: 'One', pl: 'Jeden' })]),
      element('Badge', {}, [say({ en: 'Two', pl: 'Dwa' })]),
    ]),
    Grid: element('Grid', { columns: 3 }, [
      element('Text', {}, [say({ en: 'First', pl: 'Pierwszy' })]),
      element('Text', {}, [say({ en: 'Second', pl: 'Drugi' })]),
      element('Text', {}, [say({ en: 'Third', pl: 'Trzeci' })]),
    ]),
    Divider: element('Divider'),
    Text: element('Stack', { gap: 'xs' }, [
      element('Text', {}, [say({ en: 'Default text', pl: 'Zwykły tekst' })]),
      element('Text', { tone: 'muted', size: 'sm' }, [
        say({ en: 'Muted, small', pl: 'Wyciszony, mały' }),
      ]),
      element('Text', { tone: 'error', emphasis: 'strong' }, [
        say({ en: 'Error, strong', pl: 'Błąd, pogrubiony' }),
      ]),
      element('Text', { monospace: true }, [
        say({ en: 'price_net * 1.23', pl: 'price_net * 1.23' }),
      ]),
    ]),
    Heading: element('Heading', { level: 1 }, [
      say({ en: 'Formulas', pl: 'Formuły' }),
    ]),
    Button: element('Stack', { direction: 'horizontal', gap: 'sm' }, [
      element(
        'Button',
        { variant: 'primary', icon: 'refresh' },
        [say({ en: 'Recalculate', pl: 'Przelicz' })],
        ['press'],
      ),
      element('Button', {}, [say({ en: 'Preview', pl: 'Podgląd' })], ['press']),
      element(
        'Button',
        { variant: 'text', tone: 'danger' },
        [say({ en: 'Remove', pl: 'Usuń' })],
        ['press'],
      ),
    ]),
    IconButton: element(
      'IconButton',
      { icon: 'settings', label: t('Settings', 'Ustawienia') },
      [],
      ['press'],
    ),
    TextField: element(
      'TextField',
      {
        label: t('Formula', 'Formuła'),
        value: 'price_net * (1 + vat',
        monospace: true,
        helperText: t(
          'Refer to attributes by code.',
          'Odwołuj się do atrybutów po kodzie.',
        ),
        suggestions: ['price_net * (1 + vat_rate)'],
      },
      [],
      ['change', 'submit'],
    ),
    NumberField: element(
      'NumberField',
      {
        label: t('Precision', 'Precyzja'),
        value: 2,
        minimum: 0,
        maximum: 6,
        unit: t('digits', 'cyfry'),
      },
      [],
      ['change'],
    ),
    Select: element(
      'Select',
      { label: t('Basis', 'Podstawa'), value: 'net', options },
      [],
      ['change'],
    ),
    Checkbox: element(
      'Checkbox',
      { label: t('Round half up', 'Zaokrąglaj w górę'), checked: true },
      [],
      ['change'],
    ),
    Switch: element(
      'Switch',
      { label: t('Recalculate on save', 'Przeliczaj przy zapisie') },
      [],
      ['change'],
    ),
    Badge: element(
      'Stack',
      { direction: 'horizontal', gap: 'xs', wrap: true },
      [
        element('Badge', {}, [say({ en: 'Neutral', pl: 'Neutralny' })]),
        element('Badge', { tone: 'success', icon: 'check' }, [
          say({ en: 'Valid', pl: 'Poprawny' }),
        ]),
        element('Badge', { tone: 'warning' }, [
          say({ en: 'Stale', pl: 'Nieaktualny' }),
        ]),
        element('Badge', { tone: 'error', icon: 'alert' }, [
          say({ en: 'Failed', pl: 'Błąd' }),
        ]),
      ],
    ),
    Link: element('Stack', { gap: 'xs' }, [
      element('Link', { href: '/manage/extensions/installed' }, [
        say({ en: 'Installed extensions', pl: 'Zainstalowane rozszerzenia' }),
      ]),
      element('Link', { href: 'https://attricat.org' }, [
        say({ en: 'Attricat website', pl: 'Witryna Attricat' }),
      ]),
    ]),
    Table: element(
      'Table',
      {
        label: t('Formulas', 'Formuły'),
        columns: [
          { key: 'target', label: t('Target', 'Cel'), sortable: true },
          {
            key: 'value',
            label: t('Value', 'Wartość'),
            align: 'end',
            sortable: true,
          },
        ],
        rows: [
          { id: 'gross', cells: { target: 'price_gross', value: 123 } },
          { id: 'margin', cells: { target: 'margin', value: 18.5 } },
        ],
      },
      [],
      ['rowPress'],
    ),
    EmptyState: element(
      'EmptyState',
      {
        title: t('No formulas yet', 'Brak formuł'),
        description: t(
          'Add one to compute this attribute.',
          'Dodaj formułę, aby obliczać ten atrybut.',
        ),
        icon: 'calculator',
      },
      [
        element(
          'Button',
          { variant: 'primary' },
          [say({ en: 'Add formula', pl: 'Dodaj formułę' })],
          ['press'],
          { slot: 'action' },
        ),
      ],
    ),
  } satisfies Record<keyof typeof uiKitElements, RemoteElementSerialization>;
};

/** One element drawn through a receiver, as an extension's tree would be. */
const Specimen = ({ tree }: { tree: RemoteElementSerialization }) => {
  const receiver = useMemo(() => {
    const created = new NativeReceiver(latestUiKit, {
      countNodes: () => {},
      countMutations: () => {},
      warn: (message) => console.warn(`UI kit gallery: ${message}`),
    });
    created.mutate([[MUTATION_TYPE_INSERT_CHILD, ROOT_ID, tree, 0]]);
    return created;
  }, [tree]);
  return <NativeTree receiver={receiver} />;
};

/**
 * Every UI kit element as the host draws it, in both color modes and the
 * current language, for checking adapters against the design system. The
 * page exists only in development builds.
 */
export const UiKitGallery = () => {
  const { i18n, t } = useTranslation();
  const locale = i18n.language.startsWith('pl') ? 'pl' : 'en';
  const [lastEvent, setLastEvent] = useState('');
  const specimens = useMemo(() => sample(locale, setLastEvent), [locale]);
  const themes = useMemo(
    () => designModes.map((mode) => [mode, makeTheme(mode)] as const),
    [],
  );
  return (
    <Stack spacing={3} sx={{ p: 3 }}>
      <Box>
        <Typography component="h1" variant="h5">
          {t('extensions.native.gallery.title', { version: latestUiKit })}
        </Typography>
        <Typography aria-live="polite" color="text.secondary" variant="body2">
          {lastEvent || t('extensions.native.gallery.eventsHint')}
        </Typography>
      </Box>
      {Object.entries(specimens).map(([name, tree]) => (
        <Box component="section" key={name}>
          <Typography component="h2" sx={{ mb: 1 }} variant="subtitle1">
            {name}
          </Typography>
          <Box
            sx={{
              display: 'grid',
              gap: 2,
              gridTemplateColumns: { xs: '1fr', md: '1fr 1fr' },
            }}
          >
            {themes.map(([mode, theme]) => (
              <ThemeProvider key={mode} theme={theme}>
                <ScopedCssBaseline>
                  <Paper sx={{ p: 2 }} variant="outlined">
                    <Specimen tree={tree} />
                  </Paper>
                </ScopedCssBaseline>
              </ThemeProvider>
            ))}
          </Box>
        </Box>
      ))}
    </Stack>
  );
};
