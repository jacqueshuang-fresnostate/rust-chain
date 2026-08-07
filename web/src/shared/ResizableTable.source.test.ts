/// <reference types="node" />

import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const sourceRoot = resolve(process.cwd(), 'src');

function productionTsxFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) {
      return productionTsxFiles(path);
    }
    return entry.name.endsWith('.tsx') && !entry.name.endsWith('.test.tsx') ? [path] : [];
  });
}

describe('admin table source contract', () => {
  it('keeps raw Semi Table imports and JSX inside ResizableTable only', () => {
    const rawTableFiles = productionTsxFiles(sourceRoot)
      .filter((path) => {
        const source = readFileSync(path, 'utf8');
        return /import\s*\{[^}]*\bTable\b[^}]*\}\s*from\s*['"]@douyinfe\/semi-ui['"]/.test(source) || /<Table(?:[\s>]|<)/.test(source);
      })
      .map((path) => path.slice(sourceRoot.length + 1));

    expect(rawTableFiles).toEqual(['shared/ResizableTable.tsx']);
  });
});
