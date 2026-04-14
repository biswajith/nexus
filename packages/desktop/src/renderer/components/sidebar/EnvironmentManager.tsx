import { useEffect, useState, useRef } from 'react';
import { useEnvironmentStore } from '../../stores/environment-store.js';
import type { NexusEnvironment, Variable } from '@nexus/core';
import styles from './EnvironmentManager.module.css';

interface Props {
  open: boolean;
  onClose: () => void;
}

/**
 * Full-screen modal to list, create, edit, activate, and delete environments.
 * @param props.open - When true, the overlay is shown and environments refresh.
 * @param props.onClose - Called when the user dismisses the modal.
 */
export function EnvironmentManager({ open, onClose }: Props) {
  const environments = useEnvironmentStore((s) => s.environments);
  const fetchEnvironments = useEnvironmentStore((s) => s.fetchEnvironments);
  const addEnvironment = useEnvironmentStore((s) => s.addEnvironment);
  const updateEnvironment = useEnvironmentStore((s) => s.updateEnvironment);
  const deleteEnvironment = useEnvironmentStore((s) => s.deleteEnvironment);
  const activeEnvironmentId = useEnvironmentStore((s) => s.activeEnvironmentId);
  const setActiveEnvironment = useEnvironmentStore((s) => s.setActiveEnvironment);
  const loading = useEnvironmentStore((s) => s.loading);

  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const createRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) fetchEnvironments();
  }, [open]);

  useEffect(() => {
    if (creating) createRef.current?.focus();
  }, [creating]);

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) {
      setCreating(false);
      return;
    }
    const env: NexusEnvironment = {
      id: crypto.randomUUID(),
      name,
      variables: [],
    };
    await addEnvironment(env);
    setNewName('');
    setCreating(false);
    setEditingId(env.id);
  };

  const handleDelete = async (id: string) => {
    await deleteEnvironment(id);
    if (editingId === id) setEditingId(null);
    setConfirmDeleteId(null);
  };

  const handleClose = () => {
    setEditingId(null);
    setCreating(false);
    setConfirmDeleteId(null);
    onClose();
  };

  if (!open) return null;

  const editingEnv = environments.find((e) => e.id === editingId);

  return (
    <div className={styles.overlay} onClick={handleClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.modalHeader}>
          <span className={styles.modalTitle}>
            {editingId && editingEnv ? `Edit: ${editingEnv.name}` : 'Manage Environments'}
          </span>
          <button className={styles.closeBtn} onClick={handleClose} aria-label="Close">&times;</button>
        </div>
        <div className={styles.modalBody}>
          {!editingId ? (
            <>
              <div className={styles.listHeader}>
                <span className={styles.listCount}>
                  {environments.length} environment{environments.length !== 1 ? 's' : ''}
                </span>
                <button
                  className={styles.createBtn}
                  onClick={() => { setCreating(true); setNewName(''); }}
                >
                  + New Environment
                </button>
              </div>

              {creating && (
                <div className={styles.createRow}>
                  <input
                    ref={createRef}
                    className={styles.inlineInput}
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleCreate();
                      if (e.key === 'Escape') setCreating(false);
                    }}
                    onBlur={handleCreate}
                    placeholder="Environment name"
                  />
                </div>
              )}

              {loading && <div className={styles.loading}>Loading...</div>}

              {!loading && environments.length === 0 && !creating && (
                <div className={styles.empty}>
                  <p>No environments yet.</p>
                  <p className={styles.hint}>Create one to start using variables like {'{{base_url}}'}.</p>
                </div>
              )}

              <div className={styles.envList}>
                {environments.map((env) => (
                  <div
                    key={env.id}
                    className={`${styles.envItem} ${activeEnvironmentId === env.id ? styles.envItemActive : ''}`}
                  >
                    <button
                      className={styles.envBtn}
                      onClick={() => setEditingId(env.id)}
                    >
                      <div className={styles.envInfo}>
                        <span className={styles.envName}>{env.name}</span>
                        <span className={styles.varCount}>{env.variables.length} variable{env.variables.length !== 1 ? 's' : ''}</span>
                      </div>
                      {activeEnvironmentId === env.id && (
                        <span className={styles.activeBadge}>Active</span>
                      )}
                    </button>
                    <div className={styles.envActions}>
                      {activeEnvironmentId !== env.id ? (
                        <button
                          className={styles.activateBtn}
                          onClick={() => setActiveEnvironment(env.id)}
                          title="Set as active environment"
                        >
                          Activate
                        </button>
                      ) : (
                        <button
                          className={styles.deactivateBtn}
                          onClick={() => setActiveEnvironment(null)}
                          title="Deactivate environment"
                        >
                          Deactivate
                        </button>
                      )}
                      {confirmDeleteId === env.id ? (
                        <>
                          <button
                            className={styles.confirmBtn}
                            onClick={() => handleDelete(env.id)}
                          >
                            Delete
                          </button>
                          <button
                            className={styles.cancelBtn}
                            onClick={() => setConfirmDeleteId(null)}
                          >
                            Cancel
                          </button>
                        </>
                      ) : (
                        <button
                          className={styles.deleteBtn}
                          onClick={() => setConfirmDeleteId(env.id)}
                          title="Delete environment"
                        >
                          ×
                        </button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </>
          ) : editingEnv ? (
            <EnvironmentEditor
              env={editingEnv}
              onBack={() => setEditingId(null)}
              onUpdate={updateEnvironment}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}

interface EnvironmentEditorProps {
  env: NexusEnvironment;
  onBack: () => void;
  onUpdate: (id: string, updates: Partial<NexusEnvironment>) => Promise<void>;
}

/**
 * Detail view for one environment: editable name and variable table with save.
 * @param props.env - Environment being edited.
 * @param props.onBack - Navigates back to the environment list.
 * @param props.onUpdate - Persists partial updates (e.g. name and variables).
 */
function EnvironmentEditor({ env, onBack, onUpdate }: EnvironmentEditorProps) {
  const [name, setName] = useState(env.name);
  const [variables, setVariables] = useState<Variable[]>(env.variables);
  const [dirty, setDirty] = useState(false);

  const handleNameChange = (value: string) => {
    setName(value);
    setDirty(true);
  };

  const handleVarChange = (index: number, field: keyof Variable, value: string | boolean) => {
    setVariables((prev) => {
      const updated = [...prev];
      updated[index] = { ...updated[index]!, [field]: value };
      return updated;
    });
    setDirty(true);
  };

  const addVariable = () => {
    setVariables((prev) => [
      ...prev,
      { key: '', value: '', type: 'string', enabled: true } as Variable,
    ]);
    setDirty(true);
  };

  const removeVariable = (index: number) => {
    setVariables((prev) => prev.filter((_, i) => i !== index));
    setDirty(true);
  };

  const handleSave = async () => {
    await onUpdate(env.id, { name, variables });
    setDirty(false);
  };

  return (
    <div className={styles.editor}>
      <div className={styles.editorHeader}>
        <button className={styles.backBtn} onClick={onBack}>← All Environments</button>
        <button
          className={styles.saveBtn}
          onClick={handleSave}
          disabled={!dirty}
        >
          Save Changes
        </button>
      </div>

      <div className={styles.nameRow}>
        <label className={styles.fieldLabel}>Name</label>
        <input
          className={styles.nameInput}
          value={name}
          onChange={(e) => handleNameChange(e.target.value)}
          placeholder="Environment name"
        />
      </div>

      <div className={styles.varsSection}>
        <div className={styles.varsHeader}>
          <span className={styles.varsTitle}>Variables</span>
          <button className={styles.addVarBtn} onClick={addVariable}>+ Add Variable</button>
        </div>

        {variables.length > 0 && (
          <div className={styles.varGridHeader}>
            <span />
            <span>Key</span>
            <span>Value</span>
            <span>Type</span>
            <span />
          </div>
        )}

        {variables.length === 0 && (
          <div className={styles.emptyVars}>No variables defined. Click "+ Add Variable" to create one.</div>
        )}

        {variables.map((v, i) => (
          <div key={i} className={styles.varRow}>
            <input
              type="checkbox"
              className={styles.varCheckbox}
              checked={v.enabled}
              onChange={(e) => handleVarChange(i, 'enabled', e.target.checked)}
            />
            <input
              className={styles.varInput}
              value={v.key}
              onChange={(e) => handleVarChange(i, 'key', e.target.value)}
              placeholder="Key"
            />
            <input
              className={`${styles.varInput} ${styles.varValueInput}`}
              value={v.value}
              onChange={(e) => handleVarChange(i, 'value', e.target.value)}
              placeholder="Value"
              type={v.type === 'secret' ? 'password' : 'text'}
            />
            <select
              className={styles.varTypeSelect}
              value={v.type}
              onChange={(e) => handleVarChange(i, 'type', e.target.value)}
            >
              <option value="string">String</option>
              <option value="secret">Secret</option>
            </select>
            <button className={styles.removeVarBtn} onClick={() => removeVariable(i)}>×</button>
          </div>
        ))}
      </div>
    </div>
  );
}
