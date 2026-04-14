import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import macros from 'unplugin-parcel-macros';

export default defineConfig({
  root: './src/renderer',
  plugins: [
    macros.vite(),
    react(),
  ],
  build: {
    outDir: '../../dist/renderer',
    emptyOutDir: true,
    target: ['es2022', 'chrome120'],
    cssMinify: 'lightningcss',
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (/macro-(.*)\.css$/.test(id) || /@react-spectrum\/s2\/.*\.css$/.test(id)) {
            return 's2-styles';
          }
        },
      },
    },
  },
  server: {
    port: 5173,
  },
});
