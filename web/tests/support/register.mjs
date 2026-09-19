// Test-only module hooks, loaded with `node --import` before any test file.
//
// Node strips TypeScript types from `.ts` by itself, but it does not transform JSX
// and it cannot import a stylesheet. Two synchronous hooks close that gap so that
// `react-dom/server` can render the real components under `node:test`:
//
//   * `.tsx`          -> esbuild `transformSync` (the automatic JSX runtime, the
//                        same transform Vite applies at build time);
//   * `*.module.css`  -> an identity map (`styles.shell === 'shell'`), so rendered
//                        markup carries readable, assertable class names;
//   * any other `.css`-> an empty module.
//
// esbuild is already in the tree as Vite's dependency and is declared in
// package.json at the version the lockfile resolves, so this adds no package. No
// browser, no jsdom.

import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { fileURLToPath } from 'node:url';

import { transformSync } from 'esbuild';

const CSS_MODULE = 'export default new Proxy({}, { get: (_, name) => (typeof name === "string" ? name : undefined) });\n';

registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith('.module.css')) {
      return { format: 'module', shortCircuit: true, source: CSS_MODULE };
    }
    if (url.endsWith('.css')) {
      return { format: 'module', shortCircuit: true, source: 'export {};\n' };
    }
    if (url.endsWith('.tsx')) {
      const file = fileURLToPath(url);
      const { code } = transformSync(readFileSync(file, 'utf8'), {
        loader: 'tsx',
        jsx: 'automatic',
        format: 'esm',
        target: 'es2022',
        sourcefile: file,
      });
      return { format: 'module', shortCircuit: true, source: code };
    }
    return nextLoad(url, context);
  },
});
