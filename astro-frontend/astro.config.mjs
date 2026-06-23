import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';

export default defineConfig({
  site: 'https://intel.getsulix.com',
  integrations: [mdx()],
  // content/ 在 vault 根目录，通过构建脚本复制到 astro-frontend/src/content/
  // 详见 package.json 的 build 命令
});
