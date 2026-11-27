import { useEffect, useState } from 'react';
import { Button, Modal, Spinner, TextInput } from '@doto/ui';
import { api, errorMessage } from '../api/client';
import type { HostDirsResponse } from '../api/types';
import { useT } from '../i18n';
import { Code, useNotify } from './common';

export function BrowseFolderButton({
  currentPath,
  onSelect,
}: {
  currentPath?: string;
  onSelect: (path: string) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button type="button" size="sm" variant="secondary" onClick={() => setOpen(true)}>
        📁 {t('folders.browse')}
      </Button>
      {open && (
        <FolderPickerModal
          initialPath={currentPath}
          onClose={() => setOpen(false)}
          onSelect={(p) => {
            onSelect(p);
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

export function FolderPickerModal({
  initialPath,
  onClose,
  onSelect,
}: {
  initialPath?: string;
  onClose: () => void;
  onSelect: (path: string) => void;
}) {
  const t = useT();
  const notify = useNotify();
  const [data, setData] = useState<HostDirsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [err, setErr] = useState<string | null>(null);
  const [newFolder, setNewFolder] = useState('');
  const [creating, setCreating] = useState(false);

  async function load(target?: string) {
    setLoading(true);
    setErr(null);
    try {
      const res = await api.fs.dirs(target || undefined);
      setData(res);
    } catch (e) {
      setErr(errorMessage(e));
      if (target) {
        try {
          setData(await api.fs.dirs());
        } catch {
          /* ignore */
        }
      }
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load(initialPath?.trim() || undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleMkdir() {
    if (!data?.current || !newFolder.trim()) return;
    setCreating(true);
    try {
      const r = await api.fs.mkdir(data.current, newFolder.trim());
      setNewFolder('');
      notify.ok(t('folders.mkdirDone'), r.path);
      await load(r.path);
    } catch (e) {
      notify.fail(e);
    } finally {
      setCreating(false);
    }
  }

  return (
    <Modal
      open
      wide
      onClose={onClose}
      title={t('folders.pickerTitle')}
      closeLabel={t('common.close')}
      footer={
        <>
          <Button onClick={onClose}>{t('common.cancel')}</Button>
          <Button variant="primary" disabled={!data?.current} onClick={() => data?.current && onSelect(data.current)}>
            {t('folders.selectCurrent')}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3 text-sm" data-testid="folder-picker-modal">
        <p className="text-xs text-muted">{t('folders.pickerDesc')}</p>
        {data && data.roots.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {data.roots.map((r) => (
              <button
                key={r.path}
                type="button"
                onClick={() => void load(r.path)}
                className="rounded border border-line bg-surface-2 px-2 py-1 text-xs font-medium hover:border-primary"
              >
                {r.label}
              </button>
            ))}
          </div>
        )}
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-md bg-surface-2 px-3 py-2">
          <div className="min-w-0 flex-1">
            <span className="text-xs text-muted">{t('skills.path')}: </span>
            <Code>{data?.current ?? '—'}</Code>
          </div>
          {data?.parent && (
            <Button size="sm" variant="ghost" onClick={() => void load(data.parent!)}>
              ↑ {t('folders.up')}
            </Button>
          )}
        </div>
        {err && <p className="text-xs text-danger">{err}</p>}
        {loading ? (
          <div className="flex items-center gap-2 py-6 text-muted">
            <Spinner /> {t('common.loading')}
          </div>
        ) : (
          <ul className="max-h-64 divide-y divide-line overflow-y-auto rounded-md border border-line">
            {(data?.entries ?? []).length === 0 ? (
              <li className="p-3 text-xs text-muted">{t('folders.noSubdirs')}</li>
            ) : (
              data?.entries.map((e) => (
                <li key={e.path} className="flex items-center justify-between gap-2 px-3 py-1.5 hover:bg-surface-2">
                  <button type="button" className="min-w-0 flex-1 truncate text-left font-medium hover:text-primary" onClick={() => void load(e.path)}>
                    📁 {e.name}
                  </button>
                  <Button size="sm" variant="ghost" onClick={() => onSelect(e.path)}>
                    {t('videos.select')}
                  </Button>
                </li>
              ))
            )}
          </ul>
        )}
        {data?.current && (
          <div className="flex flex-wrap items-end gap-2 pt-1">
            <TextInput
              fieldClassName="flex-1"
              label={t('folders.newFolder')}
              placeholder="DotoVideos"
              value={newFolder}
              onChange={(e) => setNewFolder(e.target.value)}
            />
            <Button size="sm" loading={creating} disabled={!newFolder.trim()} onClick={() => void handleMkdir()}>
              {t('folders.mkdir')}
            </Button>
          </div>
        )}
      </div>
    </Modal>
  );
}
