import { useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, Modal } from '@doto/ui';
import { api, errorMessage } from '../api/client';
import type { DeviceFlowStart, GithubStatus, SkillSource, UpdateCheck } from '../api/types';
import { AsyncView, Code, ExternalLink, HostOnly, KeyValue, useNotify } from './common';
import { AddFolderSourceForm, AddGithubSourceForm } from './features';
import { useI18n, useT } from '../i18n';
import { formatBytes, formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';
import type { ApiState } from '../lib/useApi';

export function SourcesTab({ state, onChanged }: { state: ApiState<SkillSource[]>; onChanged: () => void }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-4">
      <AsyncView
        state={state}
        isEmpty={(l) => l.length === 0}
        empty={<EmptyState icon="✦" title={t('sources.emptyTitle')} description={t('skills.zeroDesc')} />}
      >
        {(list) => (
          <ul className="flex flex-col gap-3" data-testid="sources-list">
            {list.map((s) => (
              <SourceRow key={s.id} source={s} onChanged={onChanged} />
            ))}
          </ul>
        )}
      </AsyncView>
      <div className="grid gap-4 md:grid-cols-2">
        <Card title={t('sources.addFolder')}>
          <AddFolderSourceForm onAdded={onChanged} />
        </Card>
        <Card title={t('sources.addGithub')}>
          <AddGithubSourceForm onAdded={onChanged} />
        </Card>
        <Card title={t('sources.importArchive')} description={t('sources.importArchiveDesc')}>
          <ImportArchiveForm onImported={onChanged} />
        </Card>
        <Card title={t('sources.uploadFolder')} description={t('sources.uploadFolderDesc')}>
          <UploadFolderForm onImported={onChanged} />
        </Card>
      </div>
    </div>
  );
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function ImportArchiveForm({ onImported }: { onImported: () => void }) {
  const t = useT();
  const notify = useNotify();
  const [file, setFile] = useState<File | null>(null);
  const [overwrite, setOverwrite] = useState(false);
  const [autoMd, setAutoMd] = useState(false);
  const [confirm, setConfirm] = useState(false);

  return (
    <HostOnly>
      <div className="flex flex-col gap-2.5 text-sm">
        <label className="ui-btn ui-btn--secondary w-fit cursor-pointer">
          📦 {file ? file.name : t('sources.pickArchive')}
          <input
            type="file"
            accept=".zip,.tar.gz,.tgz,.tar,.7z,.rar"
            className="sr-only"
            onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          />
        </label>
        <Checkbox label={t('sources.overwrite')} checked={overwrite} onChange={(e) => setOverwrite(e.target.checked)} />
        <Checkbox label={t('sources.autoMd')} checked={autoMd} onChange={(e) => setAutoMd(e.target.checked)} />
        <div>
          <Button variant="primary" disabled={!file} onClick={() => setConfirm(true)} data-testid="import-archive-btn">
            {t('sources.importArchive')}
          </Button>
        </div>
      </div>
      <ConfirmDialog
        open={confirm}
        title={t('sources.importConfirmTitle')}
        confirmLabel={t('skills.install')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirm(false)}
        onConfirm={async () => {
          if (!file) return;
          const r = await api.skills.importArchive(file, { overwrite, autoMd });
          setConfirm(false);
          setFile(null);
          notify.ok(t('sources.importedOk', { n: r.imported.length }), r.skipped.join('; ') || undefined);
          onImported();
        }}
      >
        <p>{t('sources.importConfirmBody')}</p>
        {file && (
          <p className="mt-2">
            <Code>{file.name}</Code> ({formatBytes(file.size)})
          </p>
        )}
      </ConfirmDialog>
    </HostOnly>
  );
}

export function UploadFolderForm({ onImported }: { onImported: () => void }) {
  const t = useT();
  const notify = useNotify();
  const dirRef = useRef<HTMLInputElement | null>(null);
  const [files, setFiles] = useState<File[]>([]);
  const [overwrite, setOverwrite] = useState(false);
  const [autoMd, setAutoMd] = useState(false);
  const [confirm, setConfirm] = useState(false);

  useEffect(() => {
    if (dirRef.current) {
      dirRef.current.setAttribute('webkitdirectory', '');
      dirRef.current.setAttribute('directory', '');
    }
  }, []);

  const folderName = useMemo(() => {
    const first = files[0] as (File & { webkitRelativePath?: string }) | undefined;
    const rel = first?.webkitRelativePath ?? '';
    return rel.split('/')[0] || 'uploaded-skill';
  }, [files]);

  return (
    <HostOnly>
      <div className="flex flex-col gap-2.5 text-sm">
        <label className="ui-btn ui-btn--secondary w-fit cursor-pointer">
          📂 {files.length ? `${folderName} (${files.length})` : t('sources.pickFolder')}
          <input
            ref={dirRef}
            type="file"
            multiple
            className="sr-only"
            onChange={(e) => setFiles(Array.from(e.target.files ?? []))}
          />
        </label>
        <Checkbox label={t('sources.overwrite')} checked={overwrite} onChange={(e) => setOverwrite(e.target.checked)} />
        <Checkbox label={t('sources.autoMd')} checked={autoMd} onChange={(e) => setAutoMd(e.target.checked)} />
        <div>
          <Button variant="primary" disabled={files.length === 0} onClick={() => setConfirm(true)}>
            {t('sources.uploadFolder')}
          </Button>
        </div>
      </div>
      <ConfirmDialog
        open={confirm}
        title={t('sources.importConfirmTitle')}
        confirmLabel={t('sources.uploadFolder')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirm(false)}
        onConfirm={async () => {
          const encoded = await Promise.all(
            files.map(async (f) => {
              const rel = (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name;
              const buf = new Uint8Array(await f.arrayBuffer());
              return { path: rel, contentBase64: bytesToBase64(buf) };
            }),
          );
          const r = await api.skills.uploadFolder({ folderName, files: encoded, overwrite, autoMd });
          setConfirm(false);
          setFiles([]);
          notify.ok(t('sources.importedOk', { n: r.imported.length }), r.skipped.join('; ') || undefined);
          onImported();
        }}
      >
        <p>{t('sources.importConfirmBody')}</p>
        <p className="mt-2">
          <Code>{folderName}</Code> ({files.length} files)
        </p>
      </ConfirmDialog>
    </HostOnly>
  );
}

function SourceRow({ source: s, onChanged }: { source: SkillSource; onChanged: () => void }) {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const [busy, setBusy] = useState<string | null>(null);
  const [update, setUpdate] = useState<UpdateCheck | null>(null);
  const [confirmApply, setConfirmApply] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  async function act(id: string, fn: () => Promise<unknown>, okMsg?: string) {
    setBusy(id);
    try {
      await fn();
      if (okMsg) notify.ok(okMsg);
      onChanged();
    } catch (e) {
      notify.fail(e);
    } finally {
      setBusy(null);
    }
  }

  return (
    <li>
      <Card
        title={
          <span className="flex flex-wrap items-center gap-2">
            {s.displayName}
            <Badge tone="info">{s.kind === 'github' ? 'GitHub' : t('sources.folder')}</Badge>
            {!s.enabled && <Badge tone="warning">{t('sources.paused')}</Badge>}
            {s.pinned && <Badge>{t('sources.pinned')}</Badge>}
            {s.private && <Badge>{t('sources.privateBadge')}</Badge>}
          </span>
        }
        description={<Code>{s.location}</Code>}
      >
        <KeyValue
          items={[
            [t('sources.skills'), t('sources.skillCount', { n: s.skillCount })],
            [t('sources.lastScan'), formatDate(s.lastScanAt, lang)],
            ...(s.kind === 'github'
              ? ([
                  ['Ref', s.gitRef ?? '—'],
                  ['SHA', s.currentSha ? <Code>{s.currentSha.slice(0, 12)}</Code> : '—'],
                ] as Array<[string, ReactNode]>)
              : []),
          ]}
        />
        {s.lastError && (
          <p className="mt-2 text-sm text-danger" role="alert">
            {s.lastError}
          </p>
        )}
        <div className="mt-3 flex flex-wrap gap-2">
          <HostOnly>
            <Button size="sm" loading={busy === 'enable'} onClick={() => act('enable', () => api.skills.patchSource(s.id, { enabled: !s.enabled }))}>
              {s.enabled ? t('sources.pause') : t('sources.enable')}
            </Button>
            <Button size="sm" loading={busy === 'watch'} onClick={() => act('watch', () => api.skills.patchSource(s.id, { watch: !s.watch }))}>
              {s.watch ? t('sources.unwatch') : t('sources.watchBtn')}
            </Button>
            {s.kind === 'github' && (
              <Button size="sm" loading={busy === 'pin'} onClick={() => act('pin', () => api.skills.patchSource(s.id, { pinned: !s.pinned }))}>
                {s.pinned ? t('sources.unpin') : t('sources.pin')}
              </Button>
            )}
            <Button size="sm" loading={busy === 'rescan'} onClick={() => act('rescan', () => api.skills.rescan(s.id), t('sources.rescanned'))}>
              {t('sources.rescan')}
            </Button>
            {s.kind === 'github' && (
              <Button
                size="sm"
                loading={busy === 'check'}
                onClick={async () => {
                  setBusy('check');
                  try {
                    setUpdate(await api.skills.checkUpdate(s.id));
                  } catch (e) {
                    notify.fail(e);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                {t('sources.checkUpdate')}
              </Button>
            )}
            <Button size="sm" variant="danger" onClick={() => setConfirmRemove(true)}>
              {t('sources.remove')}
            </Button>
          </HostOnly>
        </div>
      </Card>

      <Modal
        open={update !== null && !confirmApply}
        onClose={() => setUpdate(null)}
        title={t('sources.updateTitle', { name: s.displayName })}
        wide
        closeLabel={t('common.close')}
        footer={
          <>
            <Button onClick={() => setUpdate(null)}>{t('common.close')}</Button>
            {update && !update.upToDate && (
              <Button variant="primary" onClick={() => setConfirmApply(true)}>
                {t('sources.applyUpdate')}
              </Button>
            )}
          </>
        }
      >
        {update && (
          <div className="flex flex-col gap-3 text-sm">
            <p>
              {update.upToDate ? t('sources.upToDate') : t('sources.updateAvailable')} <Code>{update.currentSha?.slice(0, 12) ?? '—'}</Code> →{' '}
              <Code>{update.latestSha.slice(0, 12)}</Code>
            </p>
            {update.compareUrl && <ExternalLink href={update.compareUrl}>{t('sources.compare')}</ExternalLink>}
            {update.changedFiles.length > 0 && (
              <div className="overflow-x-auto">
                <table className="data" data-testid="changed-files">
                  <thead>
                    <tr>
                      <th>{t('sources.file')}</th>
                      <th>{t('skills.status')}</th>
                      <th>+/−</th>
                    </tr>
                  </thead>
                  <tbody>
                    {update.changedFiles.map((f) => (
                      <tr key={f.filename}>
                        <td className="font-mono text-xs break-all">{f.filename}</td>
                        <td>{f.status}</td>
                        <td className="whitespace-nowrap">
                          <span className="text-success">+{f.additions}</span> <span className="text-danger">−{f.deletions}</span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}
      </Modal>
      <ConfirmDialog
        open={confirmApply}
        title={t('sources.applyConfirmTitle')}
        confirmLabel={t('sources.applyUpdate')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmApply(false)}
        onConfirm={async () => {
          if (!update) return;
          await api.skills.applyUpdate(s.id, update.latestSha);
          setConfirmApply(false);
          setUpdate(null);
          notify.ok(t('sources.updated'));
          onChanged();
        }}
      >
        <p>{t('sources.applyConfirmBody', { n: update?.changedFiles.length ?? 0 })}</p>
      </ConfirmDialog>
      <ConfirmDialog
        open={confirmRemove}
        tone="danger"
        title={t('sources.removeConfirmTitle', { name: s.displayName })}
        confirmLabel={t('sources.remove')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmRemove(false)}
        onConfirm={async () => {
          await api.skills.removeSource(s.id);
          setConfirmRemove(false);
          notify.ok(t('sources.removed'));
          onChanged();
        }}
      >
        <p>{t('sources.removeConfirmBody')}</p>
      </ConfirmDialog>
    </li>
  );
}

export function GithubTab() {
  const t = useT();
  const notify = useNotify();
  const status = useApi('gh', () => api.github.status());
  const [flow, setFlow] = useState<DeviceFlowStart | null>(null);
  const [flowError, setFlowError] = useState<string | null>(null);
  const [confirmGh, setConfirmGh] = useState(false);
  const [confirmLogout, setConfirmLogout] = useState(false);
  const [starting, setStarting] = useState(false);
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!flow) return;
    const deadline = Date.now() + flow.expiresIn * 1000;
    let stopped = false;
    const tick = async () => {
      if (stopped) return;
      if (Date.now() > deadline) {
        setFlow(null);
        setFlowError(t('github.expired'));
        return;
      }
      try {
        await api.github.devicePoll();
        const s = await api.github.status();
        if (s.connected) {
          status.setData(s);
          setFlow(null);
          notify.ok(t('github.connected', { login: s.login ?? '' }));
          return;
        }
      } catch (e) {
        const msg = errorMessage(e);
        if (!/pending|slow/i.test(msg)) setFlowError(msg);
      }
      pollRef.current = setTimeout(tick, Math.max(5, flow.interval) * 1000);
    };
    pollRef.current = setTimeout(tick, Math.max(5, flow.interval) * 1000);
    return () => {
      stopped = true;
      if (pollRef.current) clearTimeout(pollRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [flow]);

  return (
    <AsyncView state={status}>
      {(s: GithubStatus) => (
        <div className="flex flex-col gap-4">
          <Card title={t('github.title')} description={t('github.desc')}>
            {s.connected ? (
              <div className="flex flex-col gap-3">
                <KeyValue
                  items={[
                    [t('github.account'), s.login ?? '—'],
                    [t('github.method'), s.method === 'gh' ? 'gh CLI' : s.method === 'device' ? 'Device Flow' : '—'],
                    [t('github.scopes'), s.scopes.length ? s.scopes.join(', ') : '—'],
                  ]}
                />
                <HostOnly>
                  <div>
                    <Button variant="danger" size="sm" onClick={() => setConfirmLogout(true)}>
                      {t('github.disconnect')}
                    </Button>
                  </div>
                </HostOnly>
              </div>
            ) : (
              <div className="flex flex-col gap-3">
                <p className="text-sm">{t('github.notConnected')}</p>
                <HostOnly>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      variant="primary"
                      disabled={!s.deviceFlowAvailable}
                      loading={starting}
                      onClick={async () => {
                        setStarting(true);
                        setFlowError(null);
                        try {
                          setFlow(await api.github.deviceStart());
                        } catch (e) {
                          setFlowError(errorMessage(e));
                        } finally {
                          setStarting(false);
                        }
                      }}
                    >
                      {t('github.deviceFlow')}
                    </Button>
                    <Button disabled={!s.ghCliAvailable} onClick={() => setConfirmGh(true)}>
                      {t('github.useGh')}
                    </Button>
                  </div>
                </HostOnly>
                {!s.deviceFlowAvailable && <p className="text-xs text-muted">{t('github.deviceUnavailable')}</p>}
                {!s.ghCliAvailable && <p className="text-xs text-muted">{t('github.ghUnavailable')}</p>}
                {flow && (
                  <div className="rounded-md border border-primary bg-info-bg p-4" role="status" aria-live="polite">
                    <p className="text-sm">{t('github.enterCode')}</p>
                    <p className="my-2 font-mono text-2xl font-bold tracking-widest" data-testid="device-user-code">
                      {flow.userCode}
                    </p>
                    <ExternalLink href={flow.verificationUri}>{flow.verificationUri}</ExternalLink>
                    <p className="mt-2 text-xs text-muted">{t('github.waiting')}</p>
                  </div>
                )}
                {flowError && (
                  <p className="text-sm text-danger" role="alert">
                    {flowError}
                  </p>
                )}
              </div>
            )}
          </Card>
          <ConfirmDialog
            open={confirmGh}
            title={t('github.useGhTitle')}
            confirmLabel={t('github.useGh')}
            cancelLabel={t('common.cancel')}
            onCancel={() => setConfirmGh(false)}
            onConfirm={async () => {
              await api.github.useGh();
              setConfirmGh(false);
              await status.reload();
            }}
          >
            <p>{t('github.useGhBody')}</p>
          </ConfirmDialog>
          <ConfirmDialog
            open={confirmLogout}
            tone="danger"
            title={t('github.disconnect')}
            confirmLabel={t('github.disconnect')}
            cancelLabel={t('common.cancel')}
            onCancel={() => setConfirmLogout(false)}
            onConfirm={async () => {
              await api.github.logout();
              setConfirmLogout(false);
              await status.reload();
            }}
          >
            <p>{t('github.disconnectBody')}</p>
          </ConfirmDialog>
        </div>
      )}
    </AsyncView>
  );
}
