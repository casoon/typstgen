import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ansiToHtml } from '@casoon/pages-theme/ansi';
import { url } from '@casoon/pages-theme/lib/url.ts';
import type { ShowcaseExample } from '@casoon/pages-theme/showcase';

// Everything shown here comes from examples/: each .typ source sits next to the PDF that the
// typstgen CLI compiled from it. examples/regenerate.sh rebuilds the PDFs and captured outputs.
const sources = import.meta.glob<string>('../../examples/*.{typ,txt}', {
  query: '?raw',
  import: 'default',
  eager: true,
});

function source(file: string): string {
  const found = sources[`../../examples/${file}`];
  if (found === undefined) throw new Error(`Missing fixture: examples/${file}`);
  return found;
}

/** examples/ in the repository; `astro build` and `astro dev` run in site/. */
const examplesDir = resolve(process.cwd(), '../examples');

export const pdfNames = ['letter', 'fonts', 'hello'];

export function readPdf(name: string): Buffer {
  return readFileSync(resolve(examplesDir, `${name}.pdf`));
}

/** Inline PDF preview with open and download links; the PDF is served at /examples/<name>.pdf. */
export function pdfViewer(name: string, label: string): string {
  const href = url(`examples/${name}.pdf`);
  const size = (readPdf(name).length / 1024).toFixed(1);
  return `<object data="${href}" type="application/pdf" title="${label}" style="display:block;width:100%;aspect-ratio:210/297;border:1px solid var(--border);border-radius:8px">
  <p style="margin:0;padding:16px">This browser does not show PDFs inline. Use the links below.</p>
</object>
<p style="margin:12px 0 0;font-size:14px"><a href="${href}">Open ${name}.pdf</a> · <a href="${href}" download>Download ${name}.pdf</a> (${size} KiB)</p>`;
}

const pdfExamples = [
  {
    slug: 'letter',
    title: 'Letter from a template',
    tags: ['template import', 'A4'],
    description:
      'letter.typ imports letterhead.typ, which is not next to it but in examples/templates/. typstgen finds it through template_paths in typstgen.toml.',
  },
  {
    slug: 'fonts',
    title: 'Bundled fonts',
    tags: ['fonts', 'math', 'A5'],
    description:
      'Sets each font family that typstgen bundles, including math and code. Compiled with system fonts turned off.',
  },
  {
    slug: 'hello',
    title: 'Minimal document',
    tags: ['no imports', 'A5'],
    description: 'Plain Typst markup without imports: headings, paragraphs and a list.',
  },
];

export const examples: ShowcaseExample[] = [
  ...pdfExamples.map(({ slug, title, tags, description }) => ({
    slug,
    title,
    description,
    file: `examples/${slug}.typ`,
    tags,
    input: { code: source(`${slug}.typ`), lang: 'typst' },
    output: { html: pdfViewer(slug, `${slug}.pdf, compiled by typstgen`), kind: 'panel' as const },
  })),
  {
    slug: 'missing-import',
    title: 'Missing import',
    description:
      'The imported file exists in neither the input folder nor the template directory. typstgen compile missing-import.typ --config typstgen.toml prints the Typst error to stderr and exits with code 1.',
    file: 'examples/missing-import.typ',
    tags: ['error', 'exit code 1'],
    input: { code: source('missing-import.typ'), lang: 'typst' },
    output: { html: ansiToHtml(source('missing-import.txt')), kind: 'terminal' },
  },
];
