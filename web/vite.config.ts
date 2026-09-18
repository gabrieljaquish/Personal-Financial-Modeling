import { defineConfig } from 'vite';

// Build settings here exist for one reason: the front-end bundle is compiled into
// the `pfp` executable by `rust-embed`, so two independent builds of the same
// commit must produce byte-identical `web/dist` output (ARCHITECTURE.md §9.4,
// PLAN.md §4.1 "Front-end build determinism").
//
// Deliberately absent, and each absence is a requirement rather than an oversight:
//   - no `@vitejs/plugin-react`: JSX is transformed by esbuild via the
//     `jsx: "react-jsx"` compiler option. See web/README.md for the licence
//     reason the plugin is not a dependency.
//   - no `server.port`: the served origin is `pfp-server`'s contract, not this
//     config's (ADR-006).
//   - no CDN, web-font, analytics or service-worker plugin (ADR-002, SECURITY.md §7).
export default defineConfig({
  // The server mounts the bundle at the origin root, so asset URLs stay root-relative.
  base: '/',

  // The dev server is a developer convenience only; Node is a build-time tool
  // and never ships. Bind loopback so a development machine does not publish it.
  server: {
    host: '127.0.0.1',
  },

  build: {
    // Pinned rather than inherited, so a Vite default change cannot silently
    // move the output of an otherwise unchanged commit.
    target: 'es2022',
    minify: 'esbuild',

    // Sourcemaps are off for now: they are a second, larger artifact to embed
    // and they would carry file paths into the shipped binary.
    sourcemap: false,

    // Content-hashed names only. The hash is derived from file content, so it is
    // stable across builds of the same input, and the server can serve hashed
    // assets `immutable` while `index.html` stays `no-store` (ARCHITECTURE.md §9.2).
    rollupOptions: {
      output: {
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
      },
    },

    // One stylesheet, never an inline <style>, so CSP `style-src 'self'` holds.
    cssCodeSplit: false,

    // Never inline an asset as a data: URI; every byte gets its own hashed file
    // so the build-time asset manifest the CSP is generated from is complete.
    assetsInlineLimit: 0,

    // The polyfill would be injected code that exists only to support browsers
    // this application does not target. Off keeps the entry chunk honest.
    modulePreload: { polyfill: false },

    // Gzip-size reporting is wasted work in CI and adds nothing to the artifact.
    reportCompressedSize: false,
  },
});
