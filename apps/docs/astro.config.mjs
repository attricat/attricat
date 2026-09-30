import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const polish = (label) => ({ pl: label });

export default defineConfig({
  site: 'https://docs.attricat.com',
  integrations: [
    starlight({
      title: 'Attricat Docs',
      logo: {
        dark: './src/assets/wordmark-dark.svg',
        light: './src/assets/wordmark-light.svg',
        alt: 'Attricat',
        replacesTitle: true,
      },
      customCss: ['./src/styles/brand.css'],
      components: {
        SocialIcons: './src/components/SocialIcons.astro',
      },
      description: 'Documentation for building, using, extending, and operating an Attricat catalog.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        pl: { label: 'Polski', lang: 'pl' },
      },
      social: [
        {
          icon: 'github',
          label: 'Attricat on GitHub',
          href: 'https://github.com/attricat/attricat',
        },
      ],
      sidebar: [
        {
          label: 'Get started',
          translations: polish('Na początek'),
          items: [
            {
              label: 'Introduction',
              slug: 'introduction',
              translations: polish('Wprowadzenie'),
            },
            {
              label: 'Core concepts',
              slug: 'start/concepts',
              translations: polish('Podstawowe pojęcia'),
            },
            {
              label: 'Quickstart',
              slug: 'start/quickstart',
              translations: polish('Szybki start'),
            },
          ],
        },
        {
          label: 'Use Attricat',
          translations: polish('Korzystanie z Attricat'),
          items: [
            {
              label: 'Explore entities',
              slug: 'guides/explore',
              translations: polish('Przeglądanie encji'),
            },
            {
              label: 'Search syntax',
              slug: 'guides/search-syntax',
              translations: polish('Składnia wyszukiwania'),
            },
            {
              label: 'Work with entities',
              slug: 'guides/entities',
              translations: polish('Praca z encjami'),
            },
            {
              label: 'Contexts',
              slug: 'guides/contexts',
              translations: polish('Konteksty'),
            },
            {
              label: 'Publishing',
              slug: 'guides/publishing',
              translations: polish('Publikacja'),
            },
            {
              label: 'Agents and approvals',
              slug: 'guides/agents',
              translations: polish('Agenci i zatwierdzenia'),
            },
          ],
        },
        {
          label: 'Build your catalog',
          translations: polish('Budowanie katalogu'),
          items: [
            {
              label: 'Model your catalog',
              slug: 'builders/modeling',
              translations: polish('Modelowanie katalogu'),
            },
            {
              label: 'Author a blueprint',
              slug: 'builders/blueprints',
              translations: polish('Tworzenie schematu'),
            },
            {
              label: 'Views and layouts',
              slug: 'builders/views',
              translations: polish('Widoki i układy'),
            },
            {
              label: 'Validation',
              slug: 'builders/validation',
              translations: polish('Walidacja'),
            },
            {
              label: 'Revisions and migration',
              slug: 'builders/revisions',
              translations: polish('Wersje i migracja'),
            },
            {
              label: 'Data quality rules',
              slug: 'builders/rules',
              translations: polish('Reguły jakości danych'),
            },
            {
              label: 'Workflows',
              slug: 'builders/workflows',
              translations: polish('Przepływy pracy'),
            },
            {
              label: 'Solution packs',
              slug: 'builders/solution-packs',
              translations: polish('Pakiety rozwiązań'),
            },
          ],
        },
        {
          label: 'Extensions',
          translations: polish('Rozszerzenia'),
          items: [
            {
              label: 'Install and manage',
              slug: 'builders/extensions',
              translations: polish('Instalacja i zarządzanie'),
            },
            {
              label: 'Build an extension',
              slug: 'extensions/build',
              translations: polish('Tworzenie rozszerzenia'),
            },
            {
              label: 'Manifest reference',
              slug: 'extensions/manifest',
              translations: polish('Dokumentacja manifestu'),
            },
            {
              label: 'Server runtime',
              slug: 'extensions/server',
              translations: polish('Środowisko serwerowe'),
            },
            {
              label: 'Client contributions',
              slug: 'extensions/client',
              translations: polish('Kontrybucje klienckie'),
            },
            {
              label: 'Operations and connectors',
              slug: 'extensions/operations',
              translations: polish('Operacje i konektory'),
            },
          ],
        },
        {
          label: 'Operate',
          translations: polish('Administracja'),
          items: [
            {
              label: 'Workspace administration',
              slug: 'operate/workspaces',
              translations: polish('Zarządzanie obszarem roboczym'),
            },
            {
              label: 'Deploy Attricat',
              slug: 'operate/deployment',
              translations: polish('Wdrożenie Attricat'),
            },
            {
              label: 'Monitoring',
              slug: 'operate/monitoring',
              translations: polish('Monitorowanie'),
            },
            {
              label: 'Backup and restore',
              slug: 'operate/backup',
              translations: polish('Kopie zapasowe i przywracanie'),
            },
          ],
        },
        {
          label: 'Reference',
          translations: polish('Dokumentacja techniczna'),
          items: [
            {
              label: 'Configuration',
              slug: 'reference/configuration',
              translations: polish('Konfiguracja'),
            },
            {
              label: 'Blueprint TOML',
              slug: 'reference/blueprint',
              translations: polish('Składnia TOML schematu'),
            },
            {
              label: 'Permissions',
              slug: 'reference/permissions',
              translations: polish('Uprawnienia'),
            },
            {
              label: 'CLI',
              slug: 'reference/cli',
              translations: polish('CLI'),
            },
            {
              label: 'API',
              slug: 'reference/api',
              translations: polish('API'),
            },
            {
              label: 'Events',
              slug: 'reference/events',
              translations: polish('Zdarzenia'),
            },
          ],
        },
      ],
    }),
  ],
});
