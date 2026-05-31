import '@testing-library/jest-dom/vitest';

Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
  value: () => ({
    clearRect: () => undefined,
    fillRect: () => undefined,
    measureText: (text: string) => ({ width: text.length * 8 })
  })
});
