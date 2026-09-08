import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

const polish = (label) => ({ pl: label });

export default defineConfig({
  site: 'https://docs.attricat.com',
  integrations: [
    starlight({
      title: 'Attricat Docs',
      description: 'Guides for building and operating a versioned catalog.',
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
              label: 'Contexts',
              slug: 'guides/contexts',
              translations: polish('Konteksty'),
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
              label: 'Blueprints',
              slug: 'builders/blueprints',
              translations: polish('Blueprinty'),
            },
            {
              label: 'Extensions',
              slug: 'builders/extensions',
              translations: polish('Rozszerzenia'),
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
              translations: polish('Zarządzanie przestrzenią roboczą'),
            },
          ],
        },
      ],
    }),
  ],
});
