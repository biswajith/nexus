import { useEffect, useState, useRef } from 'react';
import { useCollectionStore } from '../../stores/collection-store.js';
import { useRequestStore } from '../../stores/request-store.js';
import type { NexusRequest, NexusFolder } from '@nexus/core';
import styles from './CollectionTree.module.css';

const METHOD_COLORS: Record<string, string> = {
  GET: 'var(--nx-method-get)',
  POST: 'var(--nx-method-post)',
  PUT: 'var(--nx-method-put)',
  PATCH: 'var(--nx-method-patch)',
  DELETE: 'var(--nx-method-delete)',
  HEAD: 'var(--nx-method-head)',
  OPTIONS: 'var(--nx-method-options)',
};

function isFolderItem(item: NexusRequest | NexusFolder): item is NexusFolder {
  return 'items' in item;
}

function dirNameFromPath(p: string): string {
  return p.split('/').pop() ?? p;
}

type ContextTarget =
  | { kind: 'collection'; id: string; dirName: string; x: number; y: number }
  | { kind: 'request'; id: string; dirName: string; reqId: string; reqName: string; x: number; y: number };

export function CollectionTree() {
  const collections = useCollectionStore((s) => s.collections);
  const loadedCollections = useCollectionStore((s) => s.loadedCollections);
  const fetchCollections = useCollectionStore((s) => s.fetchCollections);
  const loadCollection = useCollectionStore((s) => s.loadCollection);
  const createCollection = useCollectionStore((s) => s.createCollection);
  const renameCollection = useCollectionStore((s) => s.renameCollection);
  const deleteCollection = useCollectionStore((s) => s.deleteCollection);
  const addRequestToCollection = useCollectionStore((s) => s.addRequest);
  const deleteRequestFromCollection = useCollectionStore((s) => s.deleteRequest);
  const loading = useCollectionStore((s) => s.loading);
  const expandedFolders = useCollectionStore((s) => s.expandedFolders);
  const toggleFolder = useCollectionStore((s) => s.toggleFolder);
  const openTab = useRequestStore((s) => s.openTab);
  const tabs = useRequestStore((s) => s.tabs);
  const activeTabId = useRequestStore((s) => s.activeTabId);

  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [contextMenu, setContextMenu] = useState<ContextTarget | null>(null);
  const [addingRequestTo, setAddingRequestTo] = useState<string | null>(null);
  const [newReqName, setNewReqName] = useState('');
  const createInputRef = useRef<HTMLInputElement>(null);
  const renameInputRef = useRef<HTMLInputElement>(null);
  const addReqInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetchCollections();
  }, []);

  useEffect(() => {
    if (creating) createInputRef.current?.focus();
  }, [creating]);

  useEffect(() => {
    if (renamingId) renameInputRef.current?.focus();
  }, [renamingId]);

  useEffect(() => {
    if (addingRequestTo) addReqInputRef.current?.focus();
  }, [addingRequestTo]);

  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener('click', close);
    return () => window.removeEventListener('click', close);
  }, [contextMenu]);

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) {
      setCreating(false);
      return;
    }
    await createCollection({
      id: crypto.randomUUID(),
      name,
      variables: [],
      items: [],
    });
    setNewName('');
    setCreating(false);
  };

  const handleRename = async () => {
    const name = renameValue.trim();
    if (!name || !renamingId) {
      setRenamingId(null);
      return;
    }
    const col = collections.find((c) => c.id === renamingId);
    if (col) {
      await renameCollection(dirNameFromPath(col.path), name);
    }
    setRenamingId(null);
  };

  const handleDelete = async (dirName: string) => {
    await deleteCollection(dirName);
    setContextMenu(null);
  };

  const handleAddNewRequest = async (dirName: string) => {
    const name = newReqName.trim() || 'New Request';
    const request: NexusRequest = {
      id: crypto.randomUUID(),
      name,
      method: 'GET',
      url: '',
      headers: [],
      params: [],
      body: { mode: 'none' },
      auth: { type: 'inherit' },
      settings: {},
    };
    await addRequestToCollection(dirName, request);
    setAddingRequestTo(null);
    setNewReqName('');
    openTab({ ...request });
  };

  const handleSaveCurrentTab = async (dirName: string) => {
    const tab = tabs.find((t) => t.id === activeTabId);
    if (!tab) return;
    const req = tab.request;
    const request: NexusRequest = {
      id: crypto.randomUUID(),
      name: req.name || req.url || 'Untitled',
      method: req.method,
      url: req.url,
      headers: req.headers,
      params: req.params,
      body: req.body,
      auth: req.auth,
      preRequestScript: req.preRequestScript,
      postResponseScript: req.postResponseScript,
      settings: req.settings,
    };
    await addRequestToCollection(dirName, request);
    setContextMenu(null);
  };

  const handleDeleteRequest = async (dirName: string, reqId: string) => {
    await deleteRequestFromCollection(dirName, reqId);
    setContextMenu(null);
  };

  const handleExpand = async (col: typeof collections[0]) => {
    const dirName = dirNameFromPath(col.path);
    toggleFolder(col.id);
    if (!expandedFolders.has(col.id) && !loadedCollections.has(dirName)) {
      await loadCollection(dirName);
    }
  };

  const handleOpenRequest = (req: NexusRequest, dirName: string) => {
    openTab(
      {
        id: req.id,
        name: req.name,
        method: req.method,
        url: req.url,
        headers: req.headers,
        params: req.params,
        body: req.body,
        auth: req.auth,
        preRequestScript: req.preRequestScript,
        postResponseScript: req.postResponseScript,
        settings: req.settings,
      },
      { dirName, requestId: req.id },
    );
  };

  const renderItems = (items: (NexusRequest | NexusFolder)[], depth: number, dirName: string) =>
    items.map((item) => {
      if (isFolderItem(item)) {
        const expanded = expandedFolders.has(item.id);
        return (
          <div key={item.id}>
            <button
              className={styles.requestItem}
              style={{ paddingLeft: `${(depth + 1) * 16 + 12}px` }}
              onClick={() => toggleFolder(item.id)}
            >
              <span className={styles.folderIcon}>{expanded ? '▾' : '▸'}</span>
              <span className={styles.folderLabel}>{item.name}</span>
            </button>
            {expanded && renderItems(item.items, depth + 1, dirName)}
          </div>
        );
      }
      return (
        <button
          key={item.id}
          className={styles.requestItem}
          style={{ paddingLeft: `${(depth + 1) * 16 + 12}px` }}
          onClick={() => handleOpenRequest(item, dirName)}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setContextMenu({
              kind: 'request',
              id: item.id,
              dirName,
              reqId: item.id,
              reqName: item.name,
              x: e.clientX,
              y: e.clientY,
            });
          }}
        >
          <span className={styles.method} style={{ color: METHOD_COLORS[item.method] ?? 'var(--nx-text-tertiary)' }}>
            {item.method}
          </span>
          <span className={styles.requestName}>{item.name}</span>
        </button>
      );
    });

  return (
    <div className={styles.tree}>
      <div className={styles.header}>
        <span className={styles.headerTitle}>Collections</span>
        <button
          className={styles.addBtn}
          aria-label="New collection"
          onClick={() => { setCreating(true); setNewName(''); }}
        >
          +
        </button>
      </div>

      {creating && (
        <div className={styles.createRow}>
          <input
            ref={createInputRef}
            className={styles.inlineInput}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreate();
              if (e.key === 'Escape') setCreating(false);
            }}
            onBlur={handleCreate}
            placeholder="Collection name"
          />
        </div>
      )}

      {loading && (
        <div className={styles.loading}>Loading collections...</div>
      )}
      {!loading && collections.length === 0 && !creating && (
        <div className={styles.empty}>
          <p>No collections yet.</p>
          <p className={styles.hint}>Click + to create a collection.</p>
        </div>
      )}

      {collections.map((col) => {
        const dirName = dirNameFromPath(col.path);
        const expanded = expandedFolders.has(col.id);
        const loaded = loadedCollections.get(dirName);

        return (
          <div key={col.id} className={styles.collectionItem}>
            {renamingId === col.id ? (
              <div className={styles.createRow}>
                <input
                  ref={renameInputRef}
                  className={styles.inlineInput}
                  value={renameValue}
                  onChange={(e) => setRenameValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleRename();
                    if (e.key === 'Escape') setRenamingId(null);
                  }}
                  onBlur={handleRename}
                />
              </div>
            ) : (
              <button
                className={styles.itemBtn}
                onClick={() => handleExpand(col)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setContextMenu({ kind: 'collection', id: col.id, dirName, x: e.clientX, y: e.clientY });
                }}
              >
                <span className={styles.folderIcon}>
                  {expanded ? '▾' : '▸'}
                </span>
                <span className={styles.itemName}>{col.name}</span>
                <span className={styles.itemActions}>
                  <button
                    className={styles.actionIcon}
                    title="More actions"
                    onClick={(e) => {
                      e.stopPropagation();
                      const rect = e.currentTarget.getBoundingClientRect();
                      setContextMenu({ kind: 'collection', id: col.id, dirName, x: rect.left, y: rect.bottom });
                    }}
                  >
                    ⋯
                  </button>
                </span>
              </button>
            )}

            {expanded && loaded && (
              <div className={styles.requestList}>
                {addingRequestTo === dirName && (
                  <div className={styles.createRow}>
                    <input
                      ref={addReqInputRef}
                      className={styles.inlineInput}
                      value={newReqName}
                      onChange={(e) => setNewReqName(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleAddNewRequest(dirName);
                        if (e.key === 'Escape') { setAddingRequestTo(null); setNewReqName(''); }
                      }}
                      onBlur={() => { if (newReqName.trim()) handleAddNewRequest(dirName); else { setAddingRequestTo(null); setNewReqName(''); } }}
                      placeholder="Request name"
                    />
                  </div>
                )}
                {loaded.items.length === 0 && !addingRequestTo ? (
                  <div className={styles.emptyRequests}>No requests</div>
                ) : (
                  renderItems(loaded.items, 0, dirName)
                )}
              </div>
            )}
          </div>
        );
      })}

      {contextMenu && (
        <div
          className={styles.contextMenu}
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.kind === 'collection' && (
            <>
              <button
                className={styles.menuItem}
                onClick={() => {
                  setAddingRequestTo(contextMenu.dirName);
                  setNewReqName('');
                  setContextMenu(null);
                  // Ensure collection is expanded
                  const col = collections.find((c) => c.id === contextMenu.id);
                  if (col && !expandedFolders.has(col.id)) {
                    void handleExpand(col);
                  }
                }}
              >
                Add Request
              </button>
              {activeTabId && (
                <button
                  className={styles.menuItem}
                  onClick={() => handleSaveCurrentTab(contextMenu.dirName)}
                >
                  Save Current Tab
                </button>
              )}
              <button
                className={styles.menuItem}
                onClick={() => {
                  const col = collections.find((c) => c.id === contextMenu.id);
                  setRenameValue(col?.name ?? '');
                  setRenamingId(contextMenu.id);
                  setContextMenu(null);
                }}
              >
                Rename
              </button>
              <button
                className={`${styles.menuItem} ${styles.menuItemDanger}`}
                onClick={() => handleDelete(contextMenu.dirName)}
              >
                Delete Collection
              </button>
            </>
          )}
          {contextMenu.kind === 'request' && (
            <>
              <button
                className={`${styles.menuItem} ${styles.menuItemDanger}`}
                onClick={() => handleDeleteRequest(contextMenu.dirName, contextMenu.reqId)}
              >
                Delete Request
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
