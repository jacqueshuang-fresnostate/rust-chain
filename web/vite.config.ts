import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

const runtimeDefines = {
  'process.env.DRAGGABLE_DEBUG': 'false'
};

export default defineConfig({
  define: runtimeDefines,
  optimizeDeps: {
    rolldownOptions: {
      transform: {
        define: runtimeDefines
      }
    }
  },
  server: {
    port: 3030,
  },
  plugins: [react()],
  test: {
    css: true,
    environment: 'jsdom',
    env: {
      VITE_API_BASE_URL: 'http://127.0.0.1:8080'
    },
    globals: true,
    setupFiles: './vitest.setup.ts',
    testTimeout: 10000
  }
});
