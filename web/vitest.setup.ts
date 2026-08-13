import '@testing-library/jest-dom/vitest';
import { configure } from '@testing-library/react';

import './src/styles.css';

// findBy*/waitFor 的轮询上限。默认 1000ms 在机器高负载时不足以等到 Semi Modal
// 完成挂载，会把时序抖动误报成断言失败。轮询命中即返回，放宽上限不影响正常用例耗时。
configure({ asyncUtilTimeout: 5000 });

if (!globalThis.ResizeObserver) {
  Object.defineProperty(globalThis, 'ResizeObserver', {
    value: class ResizeObserverMock {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
  });
}

Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
  value: () => ({
    clearRect: () => undefined,
    fillRect: () => undefined,
    measureText: (text: string) => ({ width: text.length * 8 })
  })
});
