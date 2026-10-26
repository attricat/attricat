import js from '@eslint/js';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import prettier from 'eslint-config-prettier';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist', 'node_modules'] },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommended,
      jsxA11y.flatConfigs.recommended,
      prettier,
    ],
    languageOptions: { globals: globals.browser },
    plugins: { 'react-hooks': reactHooks, 'react-refresh': reactRefresh },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              importNames: [
                'ChartColumn',
                'ChartColumnIcon',
                'Bookmark',
                'BookmarkIcon',
                'Wrench',
                'WrenchIcon',
                'Shapes',
                'ShapesIcon',
                'ClipboardCheck',
                'ClipboardCheckIcon',
                'Folder',
                'FolderIcon',
                'ListChecks',
                'ListChecksIcon',
                'Network',
                'NetworkIcon',
                'UserCog',
                'UserCogIcon',
                'User',
                'UserIcon',
                'Settings',
                'SettingsIcon',
                'Bot',
                'BotIcon',
                'Compass',
                'CompassIcon',
              ],
              message:
                'Import canonical system icons from components/systemIcons.',
              name: 'lucide-react',
            },
          ],
          patterns: [
            {
              group: ['@mui/icons-material', '@mui/icons-material/*'],
              message: 'Use lucide-react icons.',
            },
          ],
        },
      ],
      'no-restricted-syntax': [
        'error',
        {
          selector:
            'CallExpression[callee.property.name=/^toLocale(Date|Time)?String$/]',
          message:
            'Render instants with <Timestamp> or useInstantFormat from src/time; format numbers with Intl.NumberFormat.',
        },
        {
          selector:
            ':matches(NewExpression, CallExpression)[callee.object.name="Intl"][callee.property.name="DateTimeFormat"]',
          message:
            'Render instants with <Timestamp> or useInstantFormat from src/time.',
        },
        {
          // A query's or mutation's error is shown translated by its code,
          // whether read from the query, from several queries at once, or
          // from a rejected request.
          selector: [
            'MemberExpression[property.name="message"][object.type="MemberExpression"][object.property.name="error"]',
            'MemberExpression[property.name="message"][object.type="LogicalExpression"]',
            'ConditionalExpression[test.operator="instanceof"] > MemberExpression.consequent[property.name="message"]',
            'CallExpression[callee.property.name="catch"] MemberExpression[property.name="message"]',
            'Property[key.name="onError"] MemberExpression[property.name="message"]',
          ].join(', '),
          message:
            'Show request failures with errorMessage() from src/api/errors, which translates API error codes.',
        },
      ],
      'no-restricted-properties': [
        'error',
        {
          object: 'crypto',
          property: 'randomUUID',
          message:
            'Use randomUuid() from src/components/randomUuid; crypto.randomUUID exists only in secure contexts.',
        },
      ],
      'react-refresh/only-export-components': [
        'warn',
        {
          allowConstantExport: true,
          allowExportNames: [
            'Route',
            'incomingRelationshipListDisplayComponent',
            'relationshipHierarchyComponent',
          ],
        },
      ],
    },
  },
  {
    files: ['src/components/systemIcons.ts'],
    rules: { 'no-restricted-imports': 'off' },
  },
  {
    // The helper wraps crypto.randomUUID; tests run in Node, which always has it.
    files: [
      'src/components/randomUuid.ts',
      'src/**/*.test.{ts,tsx}',
      'e2e/**/*.ts',
    ],
    rules: { 'no-restricted-properties': 'off' },
  },
  {
    // The shared time module is the only place allowed to format instants.
    files: ['src/time/**/*.{ts,tsx}'],
    rules: { 'no-restricted-syntax': 'off' },
  },
  {
    files: ['src/routes/**/*.tsx'],
    rules: { 'react-refresh/only-export-components': 'off' },
  },
);
