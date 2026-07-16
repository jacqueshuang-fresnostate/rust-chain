import '@testing-library/jest-dom/vitest';
import './src/styles.css';

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
