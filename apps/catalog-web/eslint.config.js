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
            '@mui/icons-material/AssessmentOutlined',
            '@mui/icons-material/BookmarkBorderOutlined',
            '@mui/icons-material/BoltOutlined',
            '@mui/icons-material/BuildOutlined',
            '@mui/icons-material/CategoryOutlined',
            '@mui/icons-material/FactCheckOutlined',
            '@mui/icons-material/FolderOutlined',
            '@mui/icons-material/HubOutlined',
            '@mui/icons-material/ManageAccountsOutlined',
            '@mui/icons-material/PersonOutlined',
            '@mui/icons-material/SettingsOutlined',
            '@mui/icons-material/SmartToyOutlined',
            '@mui/icons-material/TravelExploreOutlined',
          ].map((name) => ({
            message:
              'Import canonical system icons from components/system-icons.',
            name,
          })),
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
    files: ['src/components/system-icons.ts'],
    rules: { 'no-restricted-imports': 'off' },
  },
  {
    files: ['src/routes/**/*.tsx'],
    rules: { 'react-refresh/only-export-components': 'off' },
  },
);
