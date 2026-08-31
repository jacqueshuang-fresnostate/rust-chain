import console from 'node:console';
import { readFile, readdir } from 'node:fs/promises';
import { gzipSync } from 'node:zlib';
import path from 'node:path';
import process from 'node:process';

const root = process.cwd();
const dist = path.join(root, 'dist');
const manifestPath = path.join(dist, '.vite', 'manifest.json');
const budget = JSON.parse(await readFile(path.join(root, 'bundle-budget.json'), 'utf8'));
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));

async function sizeOf(relativeFile) {
  const content = await readFile(path.join(dist, relativeFile));
  return { file: relativeFile, gzipBytes: gzipSync(content).byteLength, rawBytes: content.byteLength };
}

function collectStaticEntries(entryKey, collected = new Set()) {
  if (!entryKey || collected.has(entryKey)) return collected;
  collected.add(entryKey);
  for (const imported of manifest[entryKey]?.imports ?? []) collectStaticEntries(imported, collected);
  return collected;
}

function sum(sizes) {
  return sizes.reduce(
    (total, item) => ({ gzipBytes: total.gzipBytes + item.gzipBytes, rawBytes: total.rawBytes + item.rawBytes }),
    { gzipBytes: 0, rawBytes: 0 }
  );
}

function assertBudget(name, actual, limit) {
  for (const metric of ['rawBytes', 'gzipBytes']) {
    if (actual[metric] > limit[metric]) {
      throw new Error(`${name} ${metric} ${actual[metric]} 超过预算 ${limit[metric]}`);
    }
  }
}

const assetFiles = await readdir(path.join(dist, 'assets'));
const jsSizes = await Promise.all(assetFiles.filter((file) => file.endsWith('.js')).map((file) => sizeOf(`assets/${file}`)));
const cssSizes = await Promise.all(assetFiles.filter((file) => file.endsWith('.css')).map((file) => sizeOf(`assets/${file}`)));
const mainKey = Object.keys(manifest).find((key) => manifest[key].isEntry && (manifest[key].src === 'index.html' || key === 'index.html'));
if (!mainKey) throw new Error('未在 Vite manifest 中找到 Admin index.html 入口');
const initialKeys = collectStaticEntries(mainKey);
const initialFiles = [...new Set([...initialKeys].map((key) => manifest[key]?.file).filter(Boolean))];
const initialSizes = await Promise.all(initialFiles.filter((file) => file.endsWith('.js')).map(sizeOf));
const initial = sum(initialSizes);
const totalJs = sum(jsSizes);
const totalCss = sum(cssSizes);
const asyncFiles = jsSizes.filter((item) => !initialFiles.includes(item.file));
const largestAsync = asyncFiles.sort((left, right) => right.rawBytes - left.rawBytes)[0] ?? { rawBytes: 0, gzipBytes: 0, file: '-' };

assertBudget('初始 JavaScript', initial, budget.initialJavaScript);
assertBudget('最大异步 JavaScript', largestAsync, budget.largestAsyncJavaScript);
assertBudget('全部 JavaScript', totalJs, budget.totalJavaScript);
assertBudget('全部 CSS', totalCss, budget.totalCss);

const resourceConfigKey = Object.keys(manifest).find((key) => key.endsWith('src/admin/resources/resourceConfigs.tsx'));
if (!resourceConfigKey) throw new Error('未在 Vite manifest 中找到通用资源配置入口');
const staticResourceGraph = collectStaticEntries(resourceConfigKey);
const staticallyLoadedActionDomain = [...staticResourceGraph].find(
  (key) => key.includes('/resources/actions/') || key.endsWith('/actions/PredictionMarketRowActions.tsx')
);
if (staticallyLoadedActionDomain) {
  throw new Error(`通用资源配置静态导入了动作域：${staticallyLoadedActionDomain}`);
}
if (!(manifest[resourceConfigKey].dynamicImports ?? []).some((key) => key.includes('/resources/actions/'))) {
  throw new Error('通用资源配置未保留按域异步动作注册');
}

const initialContainsQuill = [...initialKeys].some((key) => key.toLowerCase().includes('quill'));
if (initialContainsQuill) throw new Error('Quill 被加入了 Admin 初始加载链路');

console.log(
  JSON.stringify(
    {
      initialJavaScript: initial,
      largestAsyncJavaScript: largestAsync,
      totalCss,
      totalJavaScript: totalJs,
      actionRegistry: '按需加载',
      quill: '非初始依赖'
    },
    null,
    2
  )
);
