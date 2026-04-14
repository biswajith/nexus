import { useState } from 'react';
import { ScriptEditor } from './ScriptEditor.js';
import styles from './ScriptPanel.module.css';

interface ScriptPanelProps {
  preRequestScript: string;
  postResponseScript: string;
  onPreRequestChange: (script: string) => void;
  onPostResponseChange: (script: string) => void;
}

type ScriptTab = 'pre-request' | 'post-response';

const SNIPPETS = [
  { label: 'Set env variable', code: 'nx.environment.set("key", "value");' },
  { label: 'Get variable', code: 'const val = nx.variables.get("key");' },
  { label: 'Test status 200', code: 'nx.test("Status is 200", () => {\n  nx.expect(nx.response.code).to.equal(200);\n});' },
  { label: 'Test has property', code: 'nx.test("Has id", () => {\n  nx.expect(nx.response.json()).to.have.property("id");\n});' },
  { label: 'Test response time', code: 'nx.test("Fast response", () => {\n  nx.expect(nx.response.responseTime).to.be.below(500);\n});' },
  { label: 'Log response', code: 'console.log(nx.response.json());' },
  { label: 'Parse & store token', code: 'const body = nx.response.json();\nnx.environment.set("token", body.access_token);' },
];

export function ScriptPanel({
  preRequestScript,
  postResponseScript,
  onPreRequestChange,
  onPostResponseChange,
}: ScriptPanelProps) {
  const [activeTab, setActiveTab] = useState<ScriptTab>('pre-request');

  const currentScript = activeTab === 'pre-request' ? preRequestScript : postResponseScript;
  const currentOnChange = activeTab === 'pre-request' ? onPreRequestChange : onPostResponseChange;

  const insertSnippet = (code: string) => {
    const newScript = currentScript ? `${currentScript}\n${code}` : code;
    currentOnChange(newScript);
  };

  return (
    <div className={styles.panel}>
      <div className={styles.tabRow}>
        <button
          className={`${styles.tabBtn} ${activeTab === 'pre-request' ? styles.activeTab : ''}`}
          onClick={() => setActiveTab('pre-request')}
        >
          Pre-request
        </button>
        <button
          className={`${styles.tabBtn} ${activeTab === 'post-response' ? styles.activeTab : ''}`}
          onClick={() => setActiveTab('post-response')}
        >
          Post-response
        </button>
      </div>
      <div className={styles.content}>
        <div className={styles.editorArea}>
          <ScriptEditor
            key={activeTab}
            value={currentScript}
            onChange={currentOnChange}
            placeholder={
              activeTab === 'pre-request'
                ? '// Pre-request script runs before the request is sent\n// Use nx.environment.set() to modify variables\n// Use nx.request.headers to modify headers'
                : '// Post-response script runs after the response is received\n// Use nx.test() to write assertions\n// Use nx.response.json() to parse the body'
            }
          />
        </div>
        <div className={styles.snippets}>
          <div className={styles.snippetsTitle}>Snippets</div>
          {SNIPPETS.map((s) => (
            <button
              key={s.label}
              className={styles.snippetBtn}
              onClick={() => insertSnippet(s.code)}
            >
              {s.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
