import { docsSchema } from '@casoon/pages-theme/content';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';

// Sources: ../docs (docs/ in the project repository). docs/PLAN.md is the internal development
// plan linked from the README, not site documentation, so it is left out.
export const collections = {
  docs: defineCollection({
    loader: glob({ pattern: ['**/*.{md,mdx}', '!PLAN.md'], base: '../docs' }),
    schema: docsSchema,
  }),
};
