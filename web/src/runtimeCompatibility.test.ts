import { describe, expect, it } from 'vitest';

import mainSource from './main.tsx?raw';
import viteConfigSource from '../vite.config.ts?raw';

describe('runtime compatibility shims', () => {
  it('loads the Semi React 19 adapter before any other app import', () => {
    const firstImport = mainSource
      .split('\n')
      .map((line) => line.trim())
      .find((line) => line.startsWith('import '));

    expect(firstImport).toBe("import '@douyinfe/semi-ui/react19-adapter';");
  });

  it('replaces react-draggable debug env access during Vite transforms', () => {
    expect(viteConfigSource).toContain("'process.env.DRAGGABLE_DEBUG': 'false'");
    expect(viteConfigSource).toContain('optimizeDeps');
    expect(viteConfigSource).toContain('rolldownOptions');
    expect(viteConfigSource).toContain('transform');
    expect(viteConfigSource).not.toContain('esbuildOptions');
  });

  it('bounds heavy jsdom suite concurrency without increasing the test deadline', () => {
    const testConfig = viteConfigSource.match(/^ {2}test: \{([\s\S]*?)^ {2}\}/m)?.[1];
    expect(testConfig).toMatch(/\bmaxWorkers:\s*2\b/);
    expect(testConfig).toMatch(/\btestTimeout:\s*20000\b/);
  });
});
