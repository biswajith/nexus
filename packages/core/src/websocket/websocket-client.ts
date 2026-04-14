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

export class WebSocketClient extends EventEmitter {
  private socket: WebSocket | null = null;
  private messages: WsMessage[] = [];
  private status: WsConnectionStatus = 'idle';
  private options: WsConnectOptions | null = null;
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private maxMessages = 1000;

  getStatus(): WsConnectionStatus {
    return this.status;
  }

  getMessages(): WsMessage[] {
    return [...this.messages];
  }

  connect(opts: WsConnectOptions): void {
    this.options = opts;
    this.reconnectAttempts = 0;
    this.doConnect();
  }

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

  clearMessages(): void {
    this.messages = [];
    this.emit('messages-cleared');
  }

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

  private addMessage(msg: WsMessage): void {
    this.messages.push(msg);
    if (this.messages.length > this.maxMessages) {
      this.messages = this.messages.slice(-this.maxMessages);
    }
  }

  private setStatus(status: WsConnectionStatus): void {
    this.status = status;
    this.emit('status', status);
  }
}
