import { defineConfig } from 'vitepress';

const REPO = 'https://github.com/vltmedia/MultiParity';

export default defineConfig({
  title: 'MultiParity',
  description: 'Self-hosted version control for game and art teams: big files, clear claims, nothing overwritten.',
  base: '/MultiParity/',
  lang: 'en',
  cleanUrls: true,
  lastUpdated: true,
  appearance: true,
  // The docs are copied from the repository's docs/ folder on every build (scripts/sync-docs.mjs).
  srcExclude: ['README.md', 'scripts/**'],
  head: [
    ['link', { rel: 'icon', type: 'image/png', href: '/MultiParity/icon.png' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.googleapis.com' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' }],
    ['link', { rel: 'stylesheet', href: 'https://fonts.googleapis.com/css2?family=Oxanium:wght@400;600;700;800&family=IBM+Plex+Sans:wght@400;500;600&family=Martian+Mono:wght@400;500&display=swap' }],
    ['meta', { property: 'og:title', content: 'MultiParity' }],
    ['meta', { property: 'og:description', content: 'Self-hosted version control for game and art teams.' }],
    ['meta', { name: 'theme-color', content: '#0094ff' }],
  ],
  themeConfig: {
    logo: { src: '/icon.png', alt: '' },
    siteTitle: 'MultiParity',
    nav: [
      { text: 'Guide', link: '/guide/', activeMatch: '/guide/' },
      { text: 'Docs', link: '/docs/SPEC', activeMatch: '/docs/' },
      { text: 'Download', link: `${REPO}/releases/latest` },
    ],
    sidebar: [
      {
        text: 'Guide',
        items: [
          { text: 'What MultiParity is', link: '/guide/' },
          { text: 'Run a server', link: '/guide/server' },
          { text: 'Use it day to day', link: '/guide/using' },
        ],
      },
      {
        text: 'Running a server',
        items: [
          { text: 'Working remotely', link: '/docs/REMOTE' },
          { text: 'Backups', link: '/docs/BACKUP' },
          { text: 'Email', link: '/docs/EMAIL' },
        ],
      },
      {
        text: 'Apps and tools',
        items: [
          { text: 'Desktop app, updates, releases', link: '/docs/DESKTOP' },
          { text: 'Code signing', link: '/docs/SIGNING' },
          { text: 'Webhooks, scripts, AI, Blender', link: '/docs/AUTOMATION' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'Product spec', link: '/docs/SPEC' },
          { text: 'HTTP API', link: '/docs/API' },
        ],
      },
    ],
    socialLinks: [{ icon: 'github', link: REPO }],
    search: { provider: 'local' },
    editLink: {
      // Runs in the browser too, so no outside variables here.
      pattern: ({ filePath }) =>
        filePath.startsWith('docs/')
          ? `https://github.com/vltmedia/MultiParity/edit/main/${filePath}`
          : `https://github.com/vltmedia/MultiParity/edit/main/site/${filePath}`,
      text: 'Edit this page on GitHub',
    },
    footer: {
      message: 'Server and apps under AGPL-3.0; mp-core, mp-client and mp-daemon under Apache-2.0.',
      copyright: 'MultiParity',
    },
    outline: { level: [2, 3], label: 'On this page' },
  },
});
