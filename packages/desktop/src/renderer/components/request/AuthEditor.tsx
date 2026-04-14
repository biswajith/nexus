import { useState } from 'react';
import type { AuthConfig, AuthType } from '@nexus/core';
import styles from './AuthEditor.module.css';

interface AuthEditorProps {
  auth: AuthConfig;
  onChange: (auth: AuthConfig) => void;
}

const AUTH_TYPES: { value: AuthType; label: string }[] = [
  { value: 'inherit', label: 'Inherit from parent' },
  { value: 'none', label: 'No Auth' },
  { value: 'bearer', label: 'Bearer Token' },
  { value: 'basic', label: 'Basic Auth' },
  { value: 'api-key', label: 'API Key' },
  { value: 'oauth2', label: 'OAuth 2.0' },
  { value: 'digest', label: 'Digest Auth' },
  { value: 'aws-sig-v4', label: 'AWS Signature v4' },
];

/** Form to configure request authentication type and type-specific credentials. */
export function AuthEditor({ auth, onChange }: AuthEditorProps) {
  const setType = (type: AuthType) => onChange({ ...auth, type });
  const setField = (key: string, value: string) => onChange({ ...auth, [key]: value });

  return (
    <div className={styles.editor}>
      <div className={styles.typeRow}>
        <label className={styles.label}>Type</label>
        <select
          className={styles.select}
          value={auth.type}
          onChange={(e) => setType(e.target.value as AuthType)}
        >
          {AUTH_TYPES.map((t) => (
            <option key={t.value} value={t.value}>{t.label}</option>
          ))}
        </select>
      </div>

      <div className={styles.fields}>
        {auth.type === 'inherit' && (
          <div className={styles.hint}>
            This request will use the auth configuration from its parent folder or collection.
          </div>
        )}

        {auth.type === 'none' && (
          <div className={styles.hint}>
            This request does not use any authentication.
          </div>
        )}

        {auth.type === 'bearer' && (
          <>
            <FieldRow label="Token" value={String(auth.token ?? '')} onChange={(v) => setField('token', v)} placeholder="{{auth_token}}" />
            <FieldRow label="Prefix" value={String(auth.prefix ?? 'Bearer')} onChange={(v) => setField('prefix', v)} placeholder="Bearer" />
          </>
        )}

        {auth.type === 'basic' && (
          <>
            <FieldRow label="Username" value={String(auth.username ?? '')} onChange={(v) => setField('username', v)} placeholder="username" />
            <FieldRow label="Password" value={String(auth.password ?? '')} onChange={(v) => setField('password', v)} placeholder="password" isSecret />
          </>
        )}

        {auth.type === 'api-key' && (
          <>
            <FieldRow label="Key" value={String(auth.key ?? '')} onChange={(v) => setField('key', v)} placeholder="X-API-Key" />
            <FieldRow label="Value" value={String(auth.value ?? '')} onChange={(v) => setField('value', v)} placeholder="your-api-key" isSecret />
            <div className={styles.fieldRow}>
              <label className={styles.fieldLabel}>Add to</label>
              <select
                className={styles.select}
                value={String(auth.addTo ?? 'header')}
                onChange={(e) => setField('addTo', e.target.value)}
              >
                <option value="header">Header</option>
                <option value="query">Query Params</option>
              </select>
            </div>
          </>
        )}

        {auth.type === 'oauth2' && (
          <>
            <FieldRow label="Access Token" value={String(auth.accessToken ?? '')} onChange={(v) => setField('accessToken', v)} placeholder="Paste or fetch token" isSecret />
            <FieldRow label="Token Type" value={String(auth.tokenType ?? 'Bearer')} onChange={(v) => setField('tokenType', v)} placeholder="Bearer" />
            <div className={styles.fieldRow}>
              <label className={styles.fieldLabel}>Add to</label>
              <select
                className={styles.select}
                value={String(auth.addTo ?? 'header')}
                onChange={(e) => setField('addTo', e.target.value)}
              >
                <option value="header">Header</option>
                <option value="query">Query Params</option>
              </select>
            </div>
            <div className={styles.separator} />
            <div className={styles.sectionTitle}>Token Configuration</div>
            <FieldRow label="Auth URL" value={String(auth.authUrl ?? '')} onChange={(v) => setField('authUrl', v)} placeholder="https://auth.example.com/authorize" />
            <FieldRow label="Token URL" value={String(auth.tokenUrl ?? '')} onChange={(v) => setField('tokenUrl', v)} placeholder="https://auth.example.com/token" />
            <FieldRow label="Client ID" value={String(auth.clientId ?? '')} onChange={(v) => setField('clientId', v)} placeholder="client-id" />
            <FieldRow label="Client Secret" value={String(auth.clientSecret ?? '')} onChange={(v) => setField('clientSecret', v)} placeholder="client-secret" isSecret />
            <FieldRow label="Scope" value={String(auth.scope ?? '')} onChange={(v) => setField('scope', v)} placeholder="read write" />
            <FieldRow label="Redirect URI" value={String(auth.redirectUri ?? '')} onChange={(v) => setField('redirectUri', v)} placeholder="http://localhost:5556/callback" />
          </>
        )}

        {auth.type === 'digest' && (
          <>
            <FieldRow label="Username" value={String(auth.username ?? '')} onChange={(v) => setField('username', v)} placeholder="username" />
            <FieldRow label="Password" value={String(auth.password ?? '')} onChange={(v) => setField('password', v)} placeholder="password" isSecret />
            <FieldRow label="Realm" value={String(auth.realm ?? '')} onChange={(v) => setField('realm', v)} placeholder="realm" />
            <FieldRow label="Nonce" value={String(auth.nonce ?? '')} onChange={(v) => setField('nonce', v)} placeholder="nonce" />
            <FieldRow label="Algorithm" value={String(auth.algorithm ?? 'MD5')} onChange={(v) => setField('algorithm', v)} placeholder="MD5" />
            <FieldRow label="QOP" value={String(auth.qop ?? 'auth')} onChange={(v) => setField('qop', v)} placeholder="auth" />
            <FieldRow label="Opaque" value={String(auth.opaque ?? '')} onChange={(v) => setField('opaque', v)} placeholder="opaque" />
          </>
        )}

        {auth.type === 'aws-sig-v4' && (
          <>
            <FieldRow label="Access Key" value={String(auth.accessKey ?? '')} onChange={(v) => setField('accessKey', v)} placeholder="AKIAIOSFODNN7EXAMPLE" />
            <FieldRow label="Secret Key" value={String(auth.secretKey ?? '')} onChange={(v) => setField('secretKey', v)} placeholder="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY" isSecret />
            <FieldRow label="Region" value={String(auth.region ?? 'us-east-1')} onChange={(v) => setField('region', v)} placeholder="us-east-1" />
            <FieldRow label="Service" value={String(auth.service ?? 'execute-api')} onChange={(v) => setField('service', v)} placeholder="execute-api" />
            <FieldRow label="Session Token" value={String(auth.sessionToken ?? '')} onChange={(v) => setField('sessionToken', v)} placeholder="optional session token" isSecret />
          </>
        )}
      </div>
    </div>
  );
}

/** Single labeled text (or password) field, with optional show/hide for secret values. */
function FieldRow({
  label,
  value,
  onChange,
  placeholder,
  isSecret = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  isSecret?: boolean;
}) {
  const [visible, setVisible] = useState(!isSecret);

  return (
    <div className={styles.fieldRow}>
      <label className={styles.fieldLabel}>{label}</label>
      <div className={styles.fieldInput}>
        <input
          className={styles.input}
          type={visible ? 'text' : 'password'}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          spellCheck={false}
          autoComplete="off"
        />
        {isSecret && (
          <button
            className={styles.toggleBtn}
            onClick={() => setVisible(!visible)}
            type="button"
            aria-label={visible ? 'Hide value' : 'Show value'}
          >
            {visible ? '🙈' : '👁'}
          </button>
        )}
      </div>
    </div>
  );
}
