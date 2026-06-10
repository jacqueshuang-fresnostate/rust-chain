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
});
