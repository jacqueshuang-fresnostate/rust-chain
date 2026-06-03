const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? '';

type TickerPayload = {
  last_price?: unknown;
  observed_at?: unknown;
  symbol?: unknown;
};

type TickerListener = (payload: { lastPrice: string; observedAt?: number; symbol: string }) => void;

function normalizeSymbol(symbol: string) {
  return symbol
    .trim()
    .split('')
    .filter((character) => /[A-Za-z0-9]/.test(character))
    .join('')
    .toUpperCase();
}

function websocketUrl(path: string) {
  if (!apiBaseUrl) {
    return path;
  }

  const url = new URL(path, apiBaseUrl);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

function parseTickerMessage(data: unknown) {
  if (typeof data !== 'string') {
    return null;
  }

  try {
    const payload = JSON.parse(data) as TickerPayload;
    if (typeof payload.symbol !== 'string' || typeof payload.last_price !== 'string') {
      return null;
    }

    return {
      symbol: normalizeSymbol(payload.symbol),
      lastPrice: payload.last_price,
      observedAt: typeof payload.observed_at === 'number' ? payload.observed_at : undefined
    };
  } catch {
    return null;
  }
}

export function subscribeMarketTicker(symbol: string, listener: TickerListener) {
  const normalizedSymbol = normalizeSymbol(symbol);
  if (!normalizedSymbol || typeof window === 'undefined' || typeof window.WebSocket === 'undefined') {
    return () => {};
  }

  const socket = new window.WebSocket(websocketUrl(`/ws/public/ticker/${normalizedSymbol}`));
  const onMessage = (event: MessageEvent) => {
    const payload = parseTickerMessage(event.data);
    if (payload && payload.symbol === normalizedSymbol) {
      listener(payload);
    }
  };

  socket.addEventListener('message', onMessage);
  return () => {
    socket.removeEventListener('message', onMessage);
    socket.close();
  };
}
