// @ts-ignore
import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const posts = defineCollection({
  loader: glob({ pattern: '*.md', base: '../content/posts' }),
  schema: z.object({
    title: z.string(),
    date: z.string(),
    status: z.enum(['published', 'draft']),
    svi: z.number(),
    color_tag: z.string(),
    is_premium: z.boolean(),
    slug: z.string(),
    summary: z.string().optional(),
    sources: z.array(z.string()).optional(),
    entities: z.array(z.string()).optional(),
    tags: z.array(z.string()).optional(),
    author: z.string().optional(),
  }),
});

export const collections = { posts };
