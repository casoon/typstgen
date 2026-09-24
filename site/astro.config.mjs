// @ts-check
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

// Project page: https://casoon.github.io/typstgen/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/typstgen',
  base: '/typstgen/',
  integrations: [
    casoonPages({
      name: 'typstgen',
      description:
        'Compiles existing .typ files to PDF with an embedded Typst engine: no typst binary, template paths from config. CLI, Rust library and Wasm.',
      repo: 'casoon/typstgen',
      version: '0.1.0',
      license: 'MIT',
      branch: 'master',
      packages: [
        { label: 'crates.io', href: 'https://crates.io/crates/typstgen' },
        { label: 'docs.rs', href: 'https://docs.rs/typstgen' },
      ],
      docsGroups: {
        'getting-started': 'Getting started',
        guides: 'Guides',
        reference: 'Reference',
      },
    }),
  ],
});
