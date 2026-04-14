import { useState, useRef, useCallback } from 'react';
import styles from './McpPanel.module.css';

type TransportType = 'stdio' | 'http';
type ConnectionStatus = 'idle' | 'connecting' | 'connected' | 'error';

interface McpTool {
  name: string;
  description?: string;
  inputSchema?: {
    type: string;
    properties?: Record<string, {
      type?: string;
      description?: string;
      enum?: string[];
      default?: unknown;
      items?: { type?: string };
    }>;
    required?: string[];
  };
}

interface McpResource {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
}

interface McpPrompt {
  name: string;
  description?: string;
  arguments?: Array<{ name: string; description?: string; required?: boolean }>;
}

interface ToolResult {
  content: Array<{ type: string; text?: string; data?: string; mimeType?: string }>;
  isError?: boolean;
  structuredContent?: unknown;
}

interface ServerInfo {
  name: string;
  version: string;
}

type ServerTab = 'tools' | 'resources' | 'prompts';

/**
 * Panel for connecting to an MCP server (stdio or HTTP), browsing tools/resources/prompts, and invoking tools with arguments.
 */
export function McpPanel() {
  const [transportType, setTransportType] = useState<TransportType>('stdio');
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');
  const [httpUrl, setHttpUrl] = useState('');
  const [envVars, setEnvVars] = useState('');
  const [status, setStatus] = useState<ConnectionStatus>('idle');
  const [error, setError] = useState<string | null>(null);
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [capabilities, setCapabilities] = useState<Record<string, unknown> | null>(null);
  const [tools, setTools] = useState<McpTool[]>([]);
  const [resources, setResources] = useState<McpResource[]>([]);
  const [prompts, setPrompts] = useState<McpPrompt[]>([]);
  const [selectedTool, setSelectedTool] = useState<McpTool | null>(null);
  const [toolArgs, setToolArgs] = useState<Record<string, string>>({});
  const [toolResult, setToolResult] = useState<ToolResult | null>(null);
  const [executing, setExecuting] = useState(false);
  const [serverTab, setServerTab] = useState<ServerTab>('tools');
  const [callHistory, setCallHistory] = useState<Array<{
    tool: string;
    args: Record<string, unknown>;
    result: ToolResult;
    timestamp: number;
    duration: number;
  }>>([]);

  const connectionId = useRef(`mcp_${Date.now()}`);

  const handleConnect = useCallback(async () => {
    setStatus('connecting');
    setError(null);
    setTools([]);
    setResources([]);
    setPrompts([]);
    setSelectedTool(null);
    setToolResult(null);
    setCallHistory([]);
    connectionId.current = `mcp_${Date.now()}`;

    try {
      const config = transportType === 'stdio'
        ? {
            type: 'stdio' as const,
            command,
            args: args.trim() ? args.split(/\s+/) : [],
            env: envVars.trim() ? parseEnvVars(envVars) : undefined,
          }
        : { type: 'http' as const, url: httpUrl };

      const info = await window.nexus.mcp.connect(connectionId.current, config) as {
        serverInfo?: ServerInfo;
        capabilities?: Record<string, unknown>;
      };

      setServerInfo(info.serverInfo ?? null);
      setCapabilities(info.capabilities ?? null);
      setStatus('connected');

      // Auto-discover tools
      try {
        const toolList = await window.nexus.mcp.listTools(connectionId.current) as McpTool[];
        setTools(toolList);
      } catch {
        // Server may not support tools
      }

      // Auto-discover resources if supported
      if (info.capabilities?.resources) {
        try {
          const resourceList = await window.nexus.mcp.listResources(connectionId.current) as McpResource[];
          setResources(resourceList);
        } catch { /* Server may not support resources */ }
      }

      // Auto-discover prompts if supported
      if (info.capabilities?.prompts) {
        try {
          const promptList = await window.nexus.mcp.listPrompts(connectionId.current) as McpPrompt[];
          setPrompts(promptList);
        } catch { /* Server may not support prompts */ }
      }
    } catch (err) {
      setStatus('error');
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [transportType, command, args, httpUrl, envVars]);

  const handleDisconnect = useCallback(async () => {
    try {
      await window.nexus.mcp.disconnect(connectionId.current);
    } catch { /* ignore */ }
    setStatus('idle');
    setServerInfo(null);
    setCapabilities(null);
    setTools([]);
    setResources([]);
    setPrompts([]);
    setSelectedTool(null);
    setToolResult(null);
  }, []);

  const handleSelectTool = useCallback((tool: McpTool) => {
    setSelectedTool(tool);
    setToolResult(null);
    const defaults: Record<string, string> = {};
    const props = tool.inputSchema?.properties;
    if (props) {
      for (const [key, schema] of Object.entries(props)) {
        if (schema.default !== undefined) {
          defaults[key] = typeof schema.default === 'string' ? schema.default : JSON.stringify(schema.default);
        } else {
          defaults[key] = '';
        }
      }
    }
    setToolArgs(defaults);
  }, []);

  const handleExecuteTool = useCallback(async () => {
    if (!selectedTool) return;
    setExecuting(true);
    setToolResult(null);

    const parsedArgs: Record<string, unknown> = {};
    const props = selectedTool.inputSchema?.properties ?? {};
    for (const [key, val] of Object.entries(toolArgs)) {
      if (val === '' && !selectedTool.inputSchema?.required?.includes(key)) continue;
      const schema = props[key];
      if (schema?.type === 'number' || schema?.type === 'integer') {
        parsedArgs[key] = Number(val);
      } else if (schema?.type === 'boolean') {
        parsedArgs[key] = val === 'true';
      } else if (schema?.type === 'array' || schema?.type === 'object') {
        try { parsedArgs[key] = JSON.parse(val); } catch { parsedArgs[key] = val; }
      } else {
        parsedArgs[key] = val;
      }
    }

    const start = performance.now();
    try {
      const result = await window.nexus.mcp.callTool(
        connectionId.current,
        selectedTool.name,
        parsedArgs,
      ) as ToolResult;
      const duration = performance.now() - start;
      setToolResult(result);
      setCallHistory((prev) => [{
        tool: selectedTool.name,
        args: parsedArgs,
        result,
        timestamp: Date.now(),
        duration,
      }, ...prev]);
    } catch (err) {
      const duration = performance.now() - start;
      const errorResult: ToolResult = {
        content: [{ type: 'text', text: err instanceof Error ? err.message : String(err) }],
        isError: true,
      };
      setToolResult(errorResult);
      setCallHistory((prev) => [{
        tool: selectedTool.name,
        args: parsedArgs,
        result: errorResult,
        timestamp: Date.now(),
        duration,
      }, ...prev]);
    } finally {
      setExecuting(false);
    }
  }, [selectedTool, toolArgs]);

  const isConnected = status === 'connected';
  const canConnect = transportType === 'stdio' ? !!command.trim() : !!httpUrl.trim();

  return (
    <div className={styles.panel}>
      {/* Connection bar */}
      <div className={styles.connectionBar}>
        <span className={`${styles.statusDot} ${statusClass(status)}`} title={status} />
        <select
          className={styles.transportSelect}
          value={transportType}
          onChange={(e) => setTransportType(e.target.value as TransportType)}
          disabled={isConnected}
        >
          <option value="stdio">stdio</option>
          <option value="http">HTTP</option>
        </select>

        {transportType === 'stdio' ? (
          <div className={styles.stdioInputs}>
            <input
              className={styles.commandInput}
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="Command (e.g. npx, node, python)"
              disabled={isConnected}
            />
            <input
              className={styles.argsInput}
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder="Args (space-separated)"
              disabled={isConnected}
            />
          </div>
        ) : (
          <input
            className={styles.urlInput}
            value={httpUrl}
            onChange={(e) => setHttpUrl(e.target.value)}
            placeholder="http://localhost:3000/mcp"
            disabled={isConnected}
          />
        )}

        {isConnected ? (
          <button type="button" className={styles.dangerBtn} onClick={handleDisconnect}>
            Disconnect
          </button>
        ) : (
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={handleConnect}
            disabled={!canConnect || status === 'connecting'}
          >
            {status === 'connecting' ? 'Connecting...' : 'Connect'}
          </button>
        )}
      </div>

      {/* Optional env vars for stdio */}
      {transportType === 'stdio' && !isConnected && (
        <div className={styles.envBar}>
          <span className={styles.envLabel}>Env:</span>
          <input
            className={styles.envInput}
            value={envVars}
            onChange={(e) => setEnvVars(e.target.value)}
            placeholder="KEY=value KEY2=value2"
          />
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className={styles.errorBanner}>
          <span>{error}</span>
          <button className={styles.dismissBtn} onClick={() => setError(null)}>×</button>
        </div>
      )}

      {/* Server info bar */}
      {isConnected && serverInfo && (
        <div className={styles.serverInfo}>
          <span className={styles.serverName}>{serverInfo.name}</span>
          <span className={styles.serverVersion}>v{serverInfo.version}</span>
          {capabilities && (
            <span className={styles.capBadges}>
              {capabilities.tools ? <span className={styles.capBadge}>Tools</span> : null}
              {capabilities.resources ? <span className={styles.capBadge}>Resources</span> : null}
              {capabilities.prompts ? <span className={styles.capBadge}>Prompts</span> : null}
            </span>
          )}
        </div>
      )}

      {/* Connected content */}
      {isConnected && (
        <div className={styles.mainContent}>
          {/* Left: server tabs */}
          <div className={styles.toolList}>
            <div className={styles.tabRow}>
              {(['tools', 'resources', 'prompts'] as const).map((tab) => (
                <button
                  key={tab}
                  className={`${styles.tabBtn} ${serverTab === tab ? styles.tabBtnActive : ''}`}
                  onClick={() => setServerTab(tab)}
                >
                  {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  <span className={styles.tabCount}>
                    {tab === 'tools' ? tools.length : tab === 'resources' ? resources.length : prompts.length}
                  </span>
                </button>
              ))}
            </div>

            <div className={styles.listScroll}>
              {serverTab === 'tools' && tools.map((tool) => (
                <button
                  key={tool.name}
                  className={`${styles.toolItem} ${selectedTool?.name === tool.name ? styles.toolItemActive : ''}`}
                  onClick={() => handleSelectTool(tool)}
                >
                  <span className={styles.toolIcon}>⚡</span>
                  <div className={styles.toolInfo}>
                    <span className={styles.toolName}>{tool.name}</span>
                    {tool.description && (
                      <span className={styles.toolDesc}>{tool.description}</span>
                    )}
                  </div>
                </button>
              ))}
              {serverTab === 'tools' && tools.length === 0 && (
                <div className={styles.emptyList}>No tools available</div>
              )}

              {serverTab === 'resources' && resources.map((res) => (
                <div key={res.uri} className={styles.resourceItem}>
                  <span className={styles.resourceName}>{res.name}</span>
                  <span className={styles.resourceUri}>{res.uri}</span>
                  {res.description && <span className={styles.resourceDesc}>{res.description}</span>}
                </div>
              ))}
              {serverTab === 'resources' && resources.length === 0 && (
                <div className={styles.emptyList}>No resources available</div>
              )}

              {serverTab === 'prompts' && prompts.map((p) => (
                <div key={p.name} className={styles.promptItem}>
                  <span className={styles.promptName}>{p.name}</span>
                  {p.description && <span className={styles.promptDesc}>{p.description}</span>}
                </div>
              ))}
              {serverTab === 'prompts' && prompts.length === 0 && (
                <div className={styles.emptyList}>No prompts available</div>
              )}
            </div>
          </div>

          {/* Right: tool tester */}
          <div className={styles.toolTester}>
            {selectedTool ? (
              <ToolTester
                tool={selectedTool}
                toolArgs={toolArgs}
                setToolArgs={setToolArgs}
                onExecute={handleExecuteTool}
                executing={executing}
                result={toolResult}
              />
            ) : (
              <div className={styles.noToolSelected}>
                <span className={styles.noToolIcon}>⚡</span>
                <p>Select a tool from the list to test it</p>
                <p className={styles.hint}>
                  You can call any tool with custom arguments and see the result instantly — no LLM needed.
                </p>
              </div>
            )}

            {/* Call history */}
            {callHistory.length > 0 && (
              <div className={styles.historySection}>
                <div className={styles.historyHeader}>
                  <span>Call History</span>
                  <button className={styles.clearBtn} onClick={() => setCallHistory([])}>Clear</button>
                </div>
                <div className={styles.historyScroll}>
                  {callHistory.map((entry, i) => (
                    <div key={i} className={`${styles.historyEntry} ${entry.result.isError ? styles.historyError : ''}`}>
                      <div className={styles.historyTop}>
                        <span className={styles.historyTool}>{entry.tool}</span>
                        <span className={styles.historyDuration}>{entry.duration.toFixed(0)}ms</span>
                        <span className={styles.historyTime}>
                          {new Date(entry.timestamp).toLocaleTimeString()}
                        </span>
                      </div>
                      <div className={styles.historyArgs}>
                        {JSON.stringify(entry.args)}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Idle state */}
      {!isConnected && !error && status !== 'connecting' && (
        <div className={styles.emptyState}>
          <span className={styles.emptyIcon}>🔌</span>
          <p>Connect to an MCP server to discover and test tools</p>
          <p className={styles.hint}>
            Supports stdio (local commands) and HTTP (remote servers).
            No LLM required — call tools directly.
          </p>
        </div>
      )}
    </div>
  );
}

interface ToolTesterProps {
  tool: McpTool;
  toolArgs: Record<string, string>;
  setToolArgs: (args: Record<string, string>) => void;
  onExecute: () => void;
  executing: boolean;
  result: ToolResult | null;
}

function ToolTester({ tool, toolArgs, setToolArgs, onExecute, executing, result }: ToolTesterProps) {
  const props = tool.inputSchema?.properties ?? {};
  const required = new Set(tool.inputSchema?.required ?? []);
  const propEntries = Object.entries(props);

  return (
    <div className={styles.testerContent}>
      <div className={styles.testerHeader}>
        <span className={styles.testerToolName}>{tool.name}</span>
        <button
          className={styles.executeBtn}
          onClick={onExecute}
          disabled={executing}
        >
          {executing ? 'Running...' : '▶ Execute'}
        </button>
      </div>

      {tool.description && (
        <div className={styles.testerDesc}>{tool.description}</div>
      )}

      {/* Arguments form */}
      {propEntries.length > 0 ? (
        <div className={styles.argsForm}>
          <div className={styles.argsTitle}>Arguments</div>
          {propEntries.map(([key, schema]) => (
            <div key={key} className={styles.argRow}>
              <div className={styles.argMeta}>
                <label className={styles.argLabel}>
                  {key}
                  {required.has(key) && <span className={styles.requiredMark}>*</span>}
                </label>
                <span className={styles.argType}>{schema.type ?? 'string'}</span>
              </div>
              {schema.description && (
                <div className={styles.argDesc}>{schema.description}</div>
              )}
              {schema.enum ? (
                <select
                  className={styles.argInput}
                  value={toolArgs[key] ?? ''}
                  onChange={(e) => setToolArgs({ ...toolArgs, [key]: e.target.value })}
                >
                  <option value="">— select —</option>
                  {schema.enum.map((v) => (
                    <option key={v} value={v}>{v}</option>
                  ))}
                </select>
              ) : schema.type === 'boolean' ? (
                <select
                  className={styles.argInput}
                  value={toolArgs[key] ?? ''}
                  onChange={(e) => setToolArgs({ ...toolArgs, [key]: e.target.value })}
                >
                  <option value="">— select —</option>
                  <option value="true">true</option>
                  <option value="false">false</option>
                </select>
              ) : (schema.type === 'object' || schema.type === 'array') ? (
                <textarea
                  className={styles.argTextarea}
                  value={toolArgs[key] ?? ''}
                  onChange={(e) => setToolArgs({ ...toolArgs, [key]: e.target.value })}
                  placeholder={`JSON ${schema.type}`}
                  rows={3}
                />
              ) : (
                <input
                  className={styles.argInput}
                  value={toolArgs[key] ?? ''}
                  onChange={(e) => setToolArgs({ ...toolArgs, [key]: e.target.value })}
                  placeholder={schema.description ?? key}
                  type={schema.type === 'number' || schema.type === 'integer' ? 'number' : 'text'}
                />
              )}
            </div>
          ))}
        </div>
      ) : (
        <div className={styles.noArgs}>This tool takes no arguments</div>
      )}

      {/* Result */}
      {result && (
        <div className={`${styles.resultSection} ${result.isError ? styles.resultError : styles.resultSuccess}`}>
          <div className={styles.resultHeader}>
            <span>{result.isError ? '✗ Error' : '✓ Success'}</span>
          </div>
          <div className={styles.resultContent}>
            {result.content.map((item, i) => (
              <div key={i} className={styles.resultItem}>
                {item.type === 'text' && (
                  <pre className={styles.resultText}>{formatResultText(item.text ?? '')}</pre>
                )}
                {item.type === 'image' && item.data && (
                  <img
                    className={styles.resultImage}
                    src={`data:${item.mimeType ?? 'image/png'};base64,${item.data}`}
                    alt="Tool result"
                  />
                )}
                {item.type !== 'text' && item.type !== 'image' && (
                  <pre className={styles.resultText}>{JSON.stringify(item, null, 2)}</pre>
                )}
              </div>
            ))}
            {result.structuredContent ? (
              <div className={styles.resultItem}>
                <div className={styles.structuredLabel}>Structured Content:</div>
                <pre className={styles.resultText}>
                  {JSON.stringify(result.structuredContent, null, 2)}
                </pre>
              </div>
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}

function statusClass(status: ConnectionStatus): string {
  switch (status) {
    case 'idle': return styles.statusIdle ?? '';
    case 'connecting': return styles.statusConnecting ?? '';
    case 'connected': return styles.statusConnected ?? '';
    case 'error': return styles.statusError ?? '';
    default: return '';
  }
}

function parseEnvVars(input: string): Record<string, string> {
  const env: Record<string, string> = {};
  for (const pair of input.split(/\s+/)) {
    const eqIdx = pair.indexOf('=');
    if (eqIdx > 0) {
      env[pair.slice(0, eqIdx)] = pair.slice(eqIdx + 1);
    }
  }
  return env;
}

function formatResultText(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}
