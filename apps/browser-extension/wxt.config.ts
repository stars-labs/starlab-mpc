import { defineConfig } from 'wxt';
import tailwindcss from '@tailwindcss/vite';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
  srcDir: 'src',
  modules: ['@wxt-dev/module-svelte'],
  vite: () => ({
    plugins: [
      wasm(),
      tailwindcss(),
    ],
    // Vite 8 + rolldown supports top-level await natively for ESM output
    // (which the MV3 module background/offscreen entrypoints use), so the
    // old vite-plugin-top-level-await is gone — under rolldown its
    // generateBundle hook crashes ("path argument must be of type string").
    // Pin the build target to esnext so esbuild/rolldown emit native TLA and
    // destructuring instead of trying (and failing) to down-level them for
    // WXT's default chrome87/es2020 target. The extension only ever runs in
    // a modern MV3 runtime, so the lower targets bought us nothing.
    build: {
      target: 'esnext',
    },
  }),
  manifest: {
    name: 'Browser Wallet',
    description: 'A secure browser extension wallet for Ethereum',
    version: '1.0.0',
    // `notifications` added for Ext-3a: chrome.notifications push
    // when someone else announces a signing session we're a
    // participant in. Without it, co-signers on MainMenu would miss
    // the invite entirely (service worker logs it but nothing
    // surfaces in the browser chrome).
    permissions: ['storage', 'tabs', 'activeTab', 'offscreen', 'notifications'],
    host_permissions: [
      'https://*/*',
      // The signal server (matches TUI's `model.rs` websocket_url).
      'wss://panda.qzz.io/*'
    ],
    icons: {
      "16": "assets/icon-16.png",
      "32": "assets/icon-32.png",
      "48": "assets/icon-48.png",
      "128": "assets/icon-128.png"
    },
    action: {
      default_popup: "popup.html",
      default_icon: {
        "16": "assets/icon-16.png",
        "32": "assets/icon-32.png"
      }
    },
    content_scripts: [
      {
        matches: ['<all_urls>'],
        js: ['content-scripts/content.js'],
        run_at: 'document_start'
      }
    ],
    background: {
      service_worker: "entrypoints/background/index.ts",
      type: "module"
    },
    content_security_policy: {
      "extension_pages": "script-src 'self' 'wasm-unsafe-eval'; object-src 'self';"
    },
    web_accessible_resources: [
      {
        resources: ['injected.js'],
        matches: ['<all_urls>']
      }
    ],
  },
});