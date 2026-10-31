import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // ArchUnitTS registers its toPassAsync() matcher on the global `expect`.
    globals: true,
    include: ['src/main/**/*.test.ts'],
  },
});
