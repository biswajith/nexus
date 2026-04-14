export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';

export interface KeyValuePair {
  key: string;
  value: string;
  enabled: boolean;
  description?: string;
}

export type BodyMode = 'none' | 'json' | 'xml' | 'text' | 'html' | 'form-data' | 'x-www-form-urlencoded' | 'binary' | 'graphql';

export interface RequestBody {
  mode: BodyMode;
  raw?: string;
  formData?: FormDataEntry[];
  urlencoded?: KeyValuePair[];
  binary?: { path: string; contentType?: string };
  graphql?: { query: string; variables?: string };
}

export interface FormDataEntry {
  key: string;
  value: string;
  type: 'text' | 'file';
  enabled: boolean;
  description?: string;
}

export type AuthType = 'none' | 'inherit' | 'api-key' | 'bearer' | 'basic' | 'oauth2' | 'digest' | 'aws-sig-v4' | 'hawk';

export interface AuthConfig {
  type: AuthType;
  [key: string]: unknown;
}

export interface RequestSettings {
  followRedirects?: boolean;
  maxRedirects?: number;
  timeout?: number;
  rejectUnauthorized?: boolean;
}

export interface NexusRequest {
  id: string;
  name: string;
  method: HttpMethod;
  url: string;
  headers: KeyValuePair[];
  params: KeyValuePair[];
  body: RequestBody;
  auth: AuthConfig;
  preRequestScript?: string;
  postResponseScript?: string;
  settings: RequestSettings;
}

export interface NexusFolder {
  id: string;
  name: string;
  description?: string;
  items: (NexusRequest | NexusFolder)[];
  auth?: AuthConfig;
  preRequestScript?: string;
  postResponseScript?: string;
  itemOrder?: string[];
}

export interface NexusCollection {
  id: string;
  name: string;
  description?: string;
  version?: string;
  variables: Variable[];
  auth?: AuthConfig;
  preRequestScript?: string;
  postResponseScript?: string;
  items: (NexusRequest | NexusFolder)[];
  itemOrder?: string[];
}

export interface Variable {
  key: string;
  value: string;
  type: 'string' | 'secret';
  enabled: boolean;
  description?: string;
}

export interface NexusEnvironment {
  id: string;
  name: string;
  variables: Variable[];
}

export interface ResponseTiming {
  dns: number;
  tcp: number;
  tls: number;
  ttfb: number;
  download: number;
  total: number;
}

export interface NexusResponse {
  status: number;
  statusText: string;
  headers: Record<string, string | string[]>;
  body: Buffer;
  bodyText: string;
  bodyJson?: unknown;
  timing: ResponseTiming;
  size: number;
}

export type ResponseFormat = 'json' | 'xml' | 'html' | 'text' | 'image' | 'pdf' | 'binary';

export interface HistoryEntry {
  id: string;
  timestamp: number;
  request: {
    method: string;
    url: string;
    headers: Record<string, string>;
    body?: string;
  };
  response: {
    status: number;
    statusText: string;
    responseTime: number;
    size: number;
  };
  collectionId?: string;
  requestId?: string;
}

export interface HistoryFilter {
  startDate?: number;
  endDate?: number;
  method?: HttpMethod;
  urlPattern?: string;
  statusCode?: number;
  limit?: number;
  offset?: number;
}

export type LogLevel = 'info' | 'warn' | 'error' | 'debug';
export type LogSource = 'script' | 'http' | 'variable' | 'system';

export interface ConsoleEntry {
  id: string;
  timestamp: number;
  level: LogLevel;
  source: LogSource;
  requestId?: string;
  message: string;
  data?: unknown;
}

export interface ConsoleFilter {
  level?: LogLevel;
  source?: LogSource;
  requestId?: string;
  search?: string;
}

export type ScopeLevel = 'local' | 'environment' | 'collection' | 'global';

export function isFolder(item: NexusRequest | NexusFolder): item is NexusFolder {
  return 'items' in item;
}

export function isRequest(item: NexusRequest | NexusFolder): item is NexusRequest {
  return 'method' in item;
}

export function createDefaultRequest(partial?: Partial<NexusRequest>): NexusRequest {
  return {
    id: crypto.randomUUID(),
    name: 'New Request',
    method: 'GET',
    url: '',
    headers: [],
    params: [],
    body: { mode: 'none' },
    auth: { type: 'inherit' },
    settings: {},
    ...partial,
  };
}

export function createDefaultEnvironment(partial?: Partial<NexusEnvironment>): NexusEnvironment {
  return {
    id: crypto.randomUUID(),
    name: 'New Environment',
    variables: [],
    ...partial,
  };
}
