import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['favicon.svg', 'apple-touch-icon.png'],
      manifest: {
        name: 'else-wer',
        short_name: 'else-wer',
        description: 'Self-hosted audiobook player',
        theme_color: '#8C7355',
        background_color: '#FDFBF8',
        display: 'standalone',
        start_url: '/',
        scope: '/',
        icons: [
          { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png', purpose: 'any' },
          { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png', purpose: 'any' },
          { src: 'maskable-icon-512x512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
        ],
      },
      workbox: {
        // Workbox's default (js,css,html) skips the bundled @fontsource woff2
        // files and icon pngs, so an offline cold start rendered with fallback
        // fonts and missing icons.
        globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
        navigateFallbackDenylist: [/^\/api\//],
        runtimeCaching: [
          {
            // Covers are static per-book assets — fine to serve stale while refetching.
            urlPattern: ({ url }) => url.pathname.startsWith('/api/covers/'),
            handler: 'StaleWhileRevalidate',
            options: {
              cacheName: 'covers-cache',
              expiration: { maxEntries: 300, maxAgeSeconds: 30 * 24 * 60 * 60 },
            },
          },
          {
            // Range-streamed audio — never let the SW attempt to cache a 206 response.
            urlPattern: ({ url }) => url.pathname.startsWith('/api/stream/'),
            handler: 'NetworkOnly',
          },
          {
            // Progress reads/writes must always hit the server — a cached response
            // here would silently desync playback position across devices.
            urlPattern: ({ url }) =>
              /^\/api\/(update_progress|get_book_progress|get_file_progress|list_inprogress)/.test(
                url.pathname,
              ),
            handler: 'NetworkOnly',
          },
        ],
      },
    }),
  ],
  server: {
    host: true,
    proxy: {
      '/api': 'http://localhost:3000',
    },
  },
})
