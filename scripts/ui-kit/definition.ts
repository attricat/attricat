/**
 * The Attricat UI kit: the only elements native extension UI may compose.
 * This file is the single source of truth. `generate.ts` turns it into
 * `contracts/ui-kit-v1.schema.json`, the web host's validators and the
 * reference pages; the SDK generates its components from that contract.
 *
 * Released versions are frozen: a later minor only adds elements, props or
 * events, each marked with the version that introduced it.
 */

/** UI kit versions, oldest first. The last is the version being built. */
export const uiKitVersions = ['1.0.0'] as const;
export type UiKitVersion = (typeof uiKitVersions)[number];

/** Host icons, as in the manifest's `HOST_ICONS`. */
export const hostIcons = [
  'alert',
  'calculator',
  'check',
  'clock',
  'download',
  'file',
  'info',
  'link',
  'refresh',
  'search',
  'send',
  'settings',
  'sparkles',
  'tag',
  'wand',
] as const;

/** Spacing steps, mapped to theme spacing by the host. */
export const spacings = ['none', 'xs', 'sm', 'md', 'lg', 'xl'] as const;
/** Semantic status tones. */
export const statusTones = ['info', 'success', 'warning', 'error'] as const;

type Text = { en: string; pl: string };

/** A prop's JSON Schema, restricted to the vocabulary the host validates. */
export type PropSchema =
  | { type: 'string'; maxLength: number; format?: 'link'; enum?: undefined }
  | { type: 'string'; enum: readonly string[] }
  | { type: 'boolean' }
  | { type: 'number'; minimum?: number; maximum?: number }
  | { type: 'integer'; minimum: number; maximum: number }
  | { type: ['number', 'null'] }
  | { type: ['string', 'null']; maxLength: number }
  | {
      type: 'array';
      maxItems: number;
      items:
        | { type: 'string'; maxLength: number }
        | {
            type: 'object';
            properties: Record<string, PropSchema>;
            required: readonly string[];
          };
    }
  | {
      type: 'object';
      maxProperties: number;
      additionalProperties: { type: ['string', 'number', 'null']; maxLength: number };
    };

export type PropDefinition = {
  schema: PropSchema;
  description: Text;
  required?: boolean;
  default?: unknown;
  since?: UiKitVersion;
};

export type EventDefinition = {
  /** The payload the host builds; extensions never choose its shape. */
  payload: Record<string, PropSchema>;
  description: Text;
  since?: UiKitVersion;
};

/**
 * What an element may contain: nothing, plain text, or kit elements
 * (optionally only some). Named slots hold one element each.
 */
export type Children =
  | { kind: 'none' }
  | { kind: 'text' }
  | { kind: 'elements'; allowed?: readonly string[] };

export type ElementDefinition = {
  /** The custom element name in the extension's Worker. */
  tag: string;
  since: UiKitVersion;
  description: Text;
  props: Record<string, PropDefinition>;
  events: Record<string, EventDefinition>;
  children: Children;
  slots: Record<string, { allowed: readonly string[]; description: Text }>;
  /** Host methods an extension may call, such as `focus`. */
  methods: readonly string[];
  /** Props that must be non-empty for assistive technology. */
  accessibility: readonly string[];
};

/** Bounds shared by several elements. */
export const uiKitLimits = {
  label: 200,
  text: 4_000,
  helperText: 400,
  placeholder: 200,
  unit: 16,
  textFieldValue: 100_000,
  suggestions: 20,
  suggestion: 400,
  selectOptions: 500,
  tableColumns: 20,
  tableRows: 1_000,
  link: 2_048,
  rows: 12,
  gridColumns: 6,
} as const;

const label = (en: string, pl: string, required = true): PropDefinition => ({
  schema: { type: 'string', maxLength: uiKitLimits.label },
  description: { en, pl },
  required,
});
const flag = (en: string, pl: string): PropDefinition => ({
  schema: { type: 'boolean' },
  description: { en, pl },
  default: false,
});
const enumeration = (
  values: readonly string[],
  fallback: string,
  en: string,
  pl: string,
): PropDefinition => ({
  schema: { type: 'string', enum: values },
  description: { en, pl },
  default: fallback,
});
const optionalText = (
  maxLength: number,
  en: string,
  pl: string,
): PropDefinition => ({
  schema: { type: 'string', maxLength },
  description: { en, pl },
});
const icon = (required = false): PropDefinition => ({
  schema: { type: 'string', enum: hostIcons },
  description: { en: 'A host icon.', pl: 'Ikona hosta.' },
  required,
});

/** Props every input shares: its label, help and state. */
const inputProps = {
  label: label(
    'The visible label, also its accessible name.',
    'Widoczna etykieta, a zarazem nazwa dostępna.',
  ),
  hideLabel: flag(
    'Hides the label visually; it stays the accessible name. Use it inside an attribute editor, whose field already shows one.',
    'Ukrywa etykietę wizualnie; pozostaje nazwą dostępną. Używaj w edytorze atrybutu, którego pole już ją pokazuje.',
  ),
  helperText: optionalText(
    uiKitLimits.helperText,
    'Help shown below the input.',
    'Pomoc pod polem.',
  ),
  error: optionalText(
    uiKitLimits.helperText,
    'An error message; the input is marked invalid while it is set.',
    'Komunikat błędu; pole jest oznaczone jako niepoprawne, dopóki jest ustawiony.',
  ),
  disabled: flag('Disables the input.', 'Wyłącza pole.'),
  required: flag('Marks the input as required.', 'Oznacza pole jako wymagane.'),
} satisfies Record<string, PropDefinition>;

const focus = ['focus'] as const;
const inputAccessibility = ['label'] as const;
const none = { kind: 'none' } as const;
const text = { kind: 'text' } as const;
const elements = { kind: 'elements' } as const;

const changeEvent = (
  value: PropSchema,
  en: string,
  pl: string,
): Record<string, EventDefinition> => ({
  change: { payload: { value }, description: { en, pl } },
});

export const uiKitElements: Record<string, ElementDefinition> = {
  Stack: {
    tag: 'attricat-stack',
    since: '1.0.0',
    description: {
      en: 'Lays out its children in a row or a column with theme spacing.',
      pl: 'Układa elementy podrzędne w wierszu lub kolumnie z odstępami motywu.',
    },
    props: {
      direction: enumeration(
        ['vertical', 'horizontal'],
        'vertical',
        'The main axis.',
        'Oś główna.',
      ),
      gap: enumeration(spacings, 'md', 'Space between children.', 'Odstęp między elementami.'),
      align: enumeration(
        ['start', 'center', 'end', 'stretch'],
        'stretch',
        'Alignment across the main axis.',
        'Wyrównanie w poprzek osi głównej.',
      ),
      justify: enumeration(
        ['start', 'center', 'end', 'between'],
        'start',
        'Distribution along the main axis.',
        'Rozkład wzdłuż osi głównej.',
      ),
      wrap: flag('Wraps a horizontal stack onto more lines.', 'Zawija poziomy stos do kolejnych linii.'),
    },
    events: {},
    children: elements,
    slots: {},
    methods: [],
    accessibility: [],
  },
  Grid: {
    tag: 'attricat-grid',
    since: '1.0.0',
    description: {
      en: 'Places its children in equal columns that collapse to one on narrow screens.',
      pl: 'Umieszcza elementy w równych kolumnach, które na wąskich ekranach łączą się w jedną.',
    },
    props: {
      columns: {
        schema: { type: 'integer', minimum: 1, maximum: uiKitLimits.gridColumns },
        description: { en: 'Columns on wide screens.', pl: 'Liczba kolumn na szerokich ekranach.' },
        default: 2,
      },
      gap: enumeration(spacings, 'md', 'Space between cells.', 'Odstęp między komórkami.'),
    },
    events: {},
    children: elements,
    slots: {},
    methods: [],
    accessibility: [],
  },
  Divider: {
    tag: 'attricat-divider',
    since: '1.0.0',
    description: { en: 'A horizontal rule.', pl: 'Pozioma linia.' },
    props: {},
    events: {},
    children: none,
    slots: {},
    methods: [],
    accessibility: [],
  },
  Text: {
    tag: 'attricat-text',
    since: '1.0.0',
    description: {
      en: 'A paragraph of plain text, already localized by the extension.',
      pl: 'Akapit zwykłego tekstu, już przetłumaczony przez rozszerzenie.',
    },
    props: {
      tone: enumeration(
        ['default', 'muted', ...statusTones],
        'default',
        'Text colour, by meaning.',
        'Kolor tekstu według znaczenia.',
      ),
      size: enumeration(['sm', 'md'], 'md', 'Text size.', 'Rozmiar tekstu.'),
      emphasis: enumeration(['regular', 'strong'], 'regular', 'Font weight.', 'Grubość czcionki.'),
      monospace: flag('Uses the code font, e.g. for formulas.', 'Używa czcionki kodu, np. dla formuł.'),
    },
    events: {},
    children: text,
    slots: {},
    methods: [],
    accessibility: [],
  },
  Heading: {
    tag: 'attricat-heading',
    since: '1.0.0',
    description: {
      en: 'A heading inside the outlet; the host owns the outlet title.',
      pl: 'Nagłówek wewnątrz miejsca osadzenia; tytuł miejsca należy do hosta.',
    },
    props: {
      level: {
        schema: { type: 'integer', minimum: 1, maximum: 3 },
        description: { en: 'Heading level within the outlet.', pl: 'Poziom nagłówka w miejscu osadzenia.' },
        default: 1,
      },
    },
    events: {},
    children: text,
    slots: {},
    methods: [],
    accessibility: [],
  },
  Button: {
    tag: 'attricat-button',
    since: '1.0.0',
    description: { en: 'A button with a text label.', pl: 'Przycisk z etykietą tekstową.' },
    props: {
      variant: enumeration(
        ['primary', 'secondary', 'text'],
        'secondary',
        'Visual emphasis.',
        'Wyróżnienie wizualne.',
      ),
      tone: enumeration(['default', 'danger'], 'default', 'A danger button confirms a destructive step.', 'Przycisk niebezpieczny zatwierdza krok nieodwracalny.'),
      icon: icon(),
      disabled: flag('Disables the button.', 'Wyłącza przycisk.'),
      loading: flag('Shows progress and disables the button.', 'Pokazuje postęp i wyłącza przycisk.'),
    },
    events: {
      press: { payload: {}, description: { en: 'The button was activated.', pl: 'Przycisk został użyty.' } },
    },
    children: text,
    slots: {},
    methods: focus,
    accessibility: [],
  },
  IconButton: {
    tag: 'attricat-icon-button',
    since: '1.0.0',
    description: { en: 'A button showing only a host icon.', pl: 'Przycisk z samą ikoną hosta.' },
    props: {
      icon: icon(true),
      label: label('The accessible name and tooltip.', 'Nazwa dostępna i podpowiedź.'),
      tone: enumeration(['default', 'danger'], 'default', 'Colour, by meaning.', 'Kolor według znaczenia.'),
      disabled: flag('Disables the button.', 'Wyłącza przycisk.'),
    },
    events: {
      press: { payload: {}, description: { en: 'The button was activated.', pl: 'Przycisk został użyty.' } },
    },
    children: none,
    slots: {},
    methods: focus,
    accessibility: ['label'],
  },
  TextField: {
    tag: 'attricat-text-field',
    since: '1.0.0',
    description: {
      en: 'A text input the host owns; it sends debounced changes.',
      pl: 'Pole tekstowe należące do hosta; wysyła zmiany z opóźnieniem.',
    },
    props: {
      ...inputProps,
      value: optionalText(uiKitLimits.textFieldValue, 'The current text.', 'Bieżący tekst.'),
      placeholder: optionalText(uiKitLimits.placeholder, 'Shown while the input is empty.', 'Widoczny, gdy pole jest puste.'),
      readOnly: flag('Shows the value without allowing changes.', 'Pokazuje wartość bez możliwości zmiany.'),
      multiline: flag('Accepts several lines.', 'Przyjmuje wiele linii.'),
      rows: {
        schema: { type: 'integer', minimum: 1, maximum: uiKitLimits.rows },
        description: { en: 'Visible lines of a multiline input.', pl: 'Widoczne linie pola wielowierszowego.' },
      },
      monospace: flag('Uses the code font.', 'Używa czcionki kodu.'),
      maxLength: {
        schema: { type: 'integer', minimum: 1, maximum: uiKitLimits.textFieldValue },
        description: { en: 'The longest text the input accepts.', pl: 'Najdłuższy tekst przyjmowany przez pole.' },
      },
      suggestions: {
        schema: {
          type: 'array',
          maxItems: uiKitLimits.suggestions,
          items: { type: 'string', maxLength: uiKitLimits.suggestion },
        },
        description: {
          en: 'Complete values the host offers below the input; choosing one replaces the text.',
          pl: 'Pełne wartości, które host proponuje pod polem; wybór zastępuje tekst.',
        },
      },
    },
    events: {
      ...changeEvent(
        { type: 'string', maxLength: uiKitLimits.textFieldValue },
        'The text changed, debounced.',
        'Tekst zmienił się (z opóźnieniem).',
      ),
      submit: {
        payload: { value: { type: 'string', maxLength: uiKitLimits.textFieldValue } },
        description: { en: 'Enter was pressed in a single-line input.', pl: 'Naciśnięto Enter w polu jednowierszowym.' },
      },
    },
    children: none,
    slots: {},
    methods: focus,
    accessibility: inputAccessibility,
  },
  NumberField: {
    tag: 'attricat-number-field',
    since: '1.0.0',
    description: { en: 'A numeric input the host owns and parses.', pl: 'Pole liczbowe należące do hosta, który je parsuje.' },
    props: {
      ...inputProps,
      value: {
        schema: { type: ['number', 'null'] },
        description: { en: 'The current number, or null when empty.', pl: 'Bieżąca liczba lub null, gdy pole jest puste.' },
      },
      minimum: { schema: { type: 'number' }, description: { en: 'The smallest accepted value.', pl: 'Najmniejsza dopuszczalna wartość.' } },
      maximum: { schema: { type: 'number' }, description: { en: 'The largest accepted value.', pl: 'Największa dopuszczalna wartość.' } },
      step: { schema: { type: 'number', minimum: 0 }, description: { en: 'The arrow-key step.', pl: 'Krok klawiszy strzałek.' } },
      unit: optionalText(uiKitLimits.unit, 'A unit shown after the input.', 'Jednostka pokazana za polem.'),
      placeholder: optionalText(uiKitLimits.placeholder, 'Shown while the input is empty.', 'Widoczny, gdy pole jest puste.'),
      readOnly: flag('Shows the value without allowing changes.', 'Pokazuje wartość bez możliwości zmiany.'),
    },
    events: changeEvent(
      { type: ['number', 'null'] },
      'The number changed, debounced; null when cleared.',
      'Liczba zmieniła się (z opóźnieniem); null po wyczyszczeniu.',
    ),
    children: none,
    slots: {},
    methods: focus,
    accessibility: inputAccessibility,
  },
  Select: {
    tag: 'attricat-select',
    since: '1.0.0',
    description: { en: 'A choice from options given as data.', pl: 'Wybór spośród opcji podanych jako dane.' },
    props: {
      ...inputProps,
      value: {
        schema: { type: ['string', 'null'], maxLength: uiKitLimits.label },
        description: { en: 'The chosen option value.', pl: 'Wartość wybranej opcji.' },
      },
      options: {
        schema: {
          type: 'array',
          maxItems: uiKitLimits.selectOptions,
          items: {
            type: 'object',
            properties: {
              value: { type: 'string', maxLength: uiKitLimits.label },
              label: { type: 'string', maxLength: uiKitLimits.label },
              disabled: { type: 'boolean' },
            },
            required: ['value', 'label'],
          },
        },
        description: { en: 'The options, in order.', pl: 'Opcje w kolejności.' },
        required: true,
      },
      placeholder: optionalText(uiKitLimits.placeholder, 'Shown while nothing is chosen.', 'Widoczny, gdy nic nie wybrano.'),
    },
    events: changeEvent(
      { type: ['string', 'null'], maxLength: uiKitLimits.label },
      'An option was chosen.',
      'Wybrano opcję.',
    ),
    children: none,
    slots: {},
    methods: focus,
    accessibility: inputAccessibility,
  },
  Checkbox: {
    tag: 'attricat-checkbox',
    since: '1.0.0',
    description: { en: 'A labelled checkbox.', pl: 'Pole wyboru z etykietą.' },
    props: {
      label: inputProps.label,
      helperText: inputProps.helperText,
      disabled: inputProps.disabled,
      checked: flag('Whether it is checked.', 'Czy jest zaznaczone.'),
    },
    events: {
      change: {
        payload: { checked: { type: 'boolean' } },
        description: { en: 'It was checked or cleared.', pl: 'Zaznaczono lub odznaczono.' },
      },
    },
    children: none,
    slots: {},
    methods: focus,
    accessibility: inputAccessibility,
  },
  Switch: {
    tag: 'attricat-switch',
    since: '1.0.0',
    description: { en: 'A labelled on/off switch that applies at once.', pl: 'Przełącznik z etykietą, działający od razu.' },
    props: {
      label: inputProps.label,
      helperText: inputProps.helperText,
      disabled: inputProps.disabled,
      checked: flag('Whether it is on.', 'Czy jest włączony.'),
    },
    events: {
      change: {
        payload: { checked: { type: 'boolean' } },
        description: { en: 'It was switched.', pl: 'Przełączono.' },
      },
    },
    children: none,
    slots: {},
    methods: focus,
    accessibility: inputAccessibility,
  },
  Badge: {
    tag: 'attricat-badge',
    since: '1.0.0',
    description: { en: 'A short status label.', pl: 'Krótka etykieta stanu.' },
    props: {
      tone: enumeration(['neutral', ...statusTones], 'neutral', 'Colour, by meaning.', 'Kolor według znaczenia.'),
      icon: icon(),
    },
    events: {},
    children: text,
    slots: {},
    methods: [],
    accessibility: [],
  },
  Link: {
    tag: 'attricat-link',
    since: '1.0.0',
    description: {
      en: 'A link to an `https:` address, opened in a new tab, or to an Attricat page by its path.',
      pl: 'Odnośnik do adresu `https:`, otwierany w nowej karcie, lub do strony Attricat według ścieżki.',
    },
    props: {
      href: {
        schema: { type: 'string', maxLength: uiKitLimits.link, format: 'link' },
        description: {
          en: 'An `https:` URL or an in-app path starting with `/`.',
          pl: 'Adres `https:` lub ścieżka w aplikacji zaczynająca się od `/`.',
        },
        required: true,
      },
    },
    events: {},
    children: text,
    slots: {},
    methods: focus,
    accessibility: [],
  },
  Table: {
    tag: 'attricat-table',
    since: '1.0.0',
    description: {
      en: 'Rows given as data; the host sorts, pages and virtualizes them.',
      pl: 'Wiersze podane jako dane; host je sortuje, stronicuje i wirtualizuje.',
    },
    props: {
      label: label('The table’s accessible name.', 'Nazwa dostępna tabeli.'),
      columns: {
        schema: {
          type: 'array',
          maxItems: uiKitLimits.tableColumns,
          items: {
            type: 'object',
            properties: {
              key: { type: 'string', maxLength: uiKitLimits.label },
              label: { type: 'string', maxLength: uiKitLimits.label },
              align: { type: 'string', enum: ['start', 'end'] },
              sortable: { type: 'boolean' },
            },
            required: ['key', 'label'],
          },
        },
        description: { en: 'The columns, in order.', pl: 'Kolumny w kolejności.' },
        required: true,
      },
      rows: {
        schema: {
          type: 'array',
          maxItems: uiKitLimits.tableRows,
          items: {
            type: 'object',
            properties: {
              id: { type: 'string', maxLength: uiKitLimits.label },
              cells: {
                type: 'object',
                maxProperties: uiKitLimits.tableColumns,
                additionalProperties: { type: ['string', 'number', 'null'], maxLength: uiKitLimits.text },
              },
            },
            required: ['id', 'cells'],
          },
        },
        description: { en: 'The rows: an ID and a value per column key.', pl: 'Wiersze: identyfikator i wartość dla każdego klucza kolumny.' },
        required: true,
      },
      emptyText: optionalText(uiKitLimits.label, 'Shown when there are no rows.', 'Widoczny, gdy nie ma wierszy.'),
    },
    events: {
      rowPress: {
        payload: { id: { type: 'string', maxLength: uiKitLimits.label } },
        description: {
          en: 'A row was activated. Rows are interactive only while a listener is set.',
          pl: 'Użyto wiersza. Wiersze są interaktywne tylko, gdy ustawiono słuchacza.',
        },
      },
    },
    children: none,
    slots: {},
    methods: [],
    accessibility: ['label'],
  },
  EmptyState: {
    tag: 'attricat-empty-state',
    since: '1.0.0',
    description: {
      en: 'Explains why there is nothing to show, with an optional action.',
      pl: 'Wyjaśnia, dlaczego nie ma nic do pokazania, z opcjonalną akcją.',
    },
    props: {
      title: label('The message.', 'Komunikat.'),
      description: optionalText(uiKitLimits.text, 'More detail.', 'Więcej szczegółów.'),
      icon: icon(),
    },
    events: {},
    children: none,
    slots: {
      action: {
        allowed: ['Button'],
        description: { en: 'A button that resolves the empty state.', pl: 'Przycisk rozwiązujący pusty stan.' },
      },
    },
    methods: [],
    accessibility: ['title'],
  },
};
