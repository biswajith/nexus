import { randomUUID } from 'node:crypto';
import { EventEmitter } from 'node:events';
import WebSocket from 'ws';

export type WsConnectionStatus =
  | 'idle'
  | 'connecting'
  | 'open'
  | 'closing'
  | 'closed'
  | 'error';

export interface WsMessage {
  id: string;
  direction: 'sent' | 'received';
  timestamp: number;
  type: 'text' | 'binary';
  data: string;
  size: number;
}

export interface WsConnectOptions {
  url: string;
  protocols?: string[];
  headers?: Record<string, string>;
  rejectUnauthorized?: boolean;
  autoReconnect?: boolean;
  reconnectDelay?: number;
  maxReconnectAttempts?: number;
}

/**
 * Event-driven WebSocket client that tracks connection state, logs messages, and supports optional auto-reconnect.
 */
export class WebSocketClient extends EventEmitter {
  private socket: WebSocket | null = null;
  private messages: WsMessage[] = [];
  private status: WsConnectionStatus = 'idle';
  private options: WsConnectOptions | null = null;
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private maxMessages = 1000;

  /**
   * Returns the current connection lifecycle status.
   * @returns The current {@link WsConnectionStatus}.
   */
  getStatus(): WsConnectionStatus {
    return this.status;
  }

  /**
   * Returns a copy of the captured sent and received message log.
   * @returns A new array containing stored {@link WsMessage} entries.
   */
  getMessages(): WsMessage[] {
    return [...this.messages];
  }

  /**
   * Stores connect options, resets reconnect attempts, and opens the WebSocket.
   * @param opts - URL, protocols, headers, TLS, and reconnect settings.
   * @returns void
   */
  connect(opts: WsConnectOptions): void {
    this.options = opts;
    this.reconnectAttempts = 0;
    this.doConnect();
  }

  /**
   * Instantiates the underlying `WebSocket` and attaches open, message, close, and error handlers (including optional reconnect).
   * @returns void
   */
  private doConnect(): void {
    if (!this.options) return;
    const { url, protocols, headers, rejectUnauthorized } = this.options;

    this.setStatus('connecting');

    this.socket = new WebSocket(url, protocols ?? [], {
      headers: headers ?? {},
      rejectUnauthorized: rejectUnauthorized ?? true,
    });

    this.socket.on('open', () => {
      this.reconnectAttempts = 0;
      this.setStatus('open');
    });

    this.socket.on('message', (data: WebSocket.RawData, isBinary: boolean) => {
      const rawBuf = WebSocketClient.rawDataToBuffer(data);
      const strData = isBinary ? rawBuf.toString('hex') : rawBuf.toString('utf8');
      const msg: WsMessage = {
        id: randomUUID(),
        direction: 'received',
        timestamp: Date.now(),
        type: isBinary ? 'binary' : 'text',
        data: strData,
        size: rawBuf.length,
      };
      this.addMessage(msg);
      this.emit('message', msg);
    });

    this.socket.on('close', (code: number, reason: Buffer) => {
      this.setStatus('closed');
      this.emit('close', { code, reason: reason.toString() });

      if (this.options?.autoReconnect) {
        const maxAttempts = this.options.maxReconnectAttempts ?? 5;
        if (this.reconnectAttempts < maxAttempts) {
          this.reconnectAttempts++;
          const delay = this.options.reconnectDelay ?? 3000;
          this.reconnectTimer = setTimeout(() => this.doConnect(), delay);
        }
      }
    });

    this.socket.on('error', (err: Error) => {
      this.setStatus('error');
      this.emit('error', err);
    });
  }

  /**
   * Sends a UTF-8 text frame and records it in the message log.
   * @param data - Text payload to send.
   * @returns The recorded {@link WsMessage} for this outbound frame.
   */
  send(data: string): WsMessage {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected');
    }
    this.socket.send(data);
    const msg: WsMessage = {
      id: randomUUID(),
      direction: 'sent',
      timestamp: Date.now(),
      type: 'text',
      data,
      size: Buffer.byteLength(data, 'utf8'),
    };
    this.addMessage(msg);
    return msg;
  }

  /**
   * Sends a binary frame and records it in the message log (payload stored as hex).
   * @param data - Binary payload to send.
   * @returns The recorded {@link WsMessage} for this outbound frame.
   */
  sendBinary(data: Buffer): WsMessage {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected');
    }
    this.socket.send(data);
    const msg: WsMessage = {
      id: randomUUID(),
      direction: 'sent',
      timestamp: Date.now(),
      type: 'binary',
      data: data.toString('hex'),
      size: data.length,
    };
    this.addMessage(msg);
    return msg;
  }

  /**
   * Disables auto-reconnect, closes the socket, and clears any pending reconnect timer.
   * @param code - WebSocket close code (defaults to 1000).
   * @param reason - Optional close reason string.
   * @returns void
   */
  disconnect(code?: number, reason?: string): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.options = this.options ? { ...this.options, autoReconnect: false } : null;

    if (this.socket) {
      this.setStatus('closing');
      this.socket.close(code ?? 1000, reason);
    }
  }

  /**
   * Clears the in-memory message history and emits `messages-cleared`.
   * @returns void
   */
  clearMessages(): void {
    this.messages = [];
    this.emit('messages-cleared');
  }

  /**
   * Normalizes `ws` raw message data to a Node.js {@link Buffer}.
   * @param data - Raw payload from a `message` event.
   * @returns A buffer representing the incoming bytes.
   */
  private static rawDataToBuffer(data: WebSocket.RawData): Buffer {
    if (Buffer.isBuffer(data)) {
      return data;
    }
    if (Array.isArray(data)) {
      return Buffer.concat(data);
    }
    if (data instanceof ArrayBuffer) {
      return Buffer.from(data);
    }
    return Buffer.from(data);
  }

  /**
   * Appends a message and trims the oldest entries when the log exceeds the max size.
   * @param msg - Message to append.
   * @returns void
   */
  private addMessage(msg: WsMessage): void {
    this.messages.push(msg);
    if (this.messages.length > this.maxMessages) {
      this.messages = this.messages.slice(-this.maxMessages);
    }
  }

  /**
   * Updates the internal status and emits a `status` event with the new value.
   * @param status - Next lifecycle status.
   * @returns void
   */
  private setStatus(status: WsConnectionStatus): void {
    this.status = status;
    this.emit('status', status);
  }
}
