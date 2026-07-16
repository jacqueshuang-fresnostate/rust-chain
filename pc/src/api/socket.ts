type MessageHandler = (data: any) => void

export class WebSocketClient {
  private url: string
  private socket: WebSocket | null = null
  private shouldReconnect: boolean = true
  private reconnectInterval: number = 3000
  private heartbeatInterval: number = 30000
  private heartbeatTimer: any = null
  private handlers: Set<MessageHandler> = new Set()

  constructor(url: string) {
    this.url = url
  }

  public connect() {
    this.socket = new WebSocket(this.url)

    this.socket.onopen = () => {
      console.log('WebSocket Connected')
      this.startHeartbeat()
    }

    this.socket.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        // Handle heartbeat response if necessary
        if (data.type === 'pong') return

        this.handlers.forEach(handler => handler(data))
      } catch (e) {
        console.warn('Failed to parse WS message', event.data)
      }
    }

    this.socket.onclose = () => {
      console.log('WebSocket Closed')
      this.stopHeartbeat()
      if (this.shouldReconnect) {
        setTimeout(() => this.connect(), this.reconnectInterval)
      }
    }

    this.socket.onerror = (error) => {
      console.error('WebSocket Error', error)
    }
  }

  public disconnect() {
    this.shouldReconnect = false
    this.stopHeartbeat()
    if (this.socket) {
      this.socket.close()
    }
  }

  public send(data: any) {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(data))
    } else {
      console.warn('WebSocket not open, cannot send', data)
    }
  }

  public addHandler(handler: MessageHandler) {
    this.handlers.add(handler)
  }

  public removeHandler(handler: MessageHandler) {
    this.handlers.delete(handler)
  }

  private startHeartbeat() {
    this.stopHeartbeat()
    this.heartbeatTimer = setInterval(() => {
        this.send({ type: 'ping' })
    }, this.heartbeatInterval)
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }
}
