import { useEffect, useState } from 'react';
import { Badge, Button, Card, Checkbox, ConfirmDialog, Select, TextInput, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { ManagedTunnel, ManagedTunnelKind, ManagedTunnelReq, TunnelMode, TunnelStatus } from '../api/types';
import { AsyncView, Code, ExternalLink, HostOnly, KeyValue, LogBox, useNotify } from './common';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';
import { useCaps } from '../state/session';

export function TunnelCard() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const status = useApi('tunnel', () => api.tunnel.status());
  const [confirm, setConfirm] = useState<'start' | 'deleteToken' | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  useServerEvent('tunnel', (s) => {
    if (s && typeof s === 'object' && 'running' in s && Array.isArray(s.logTail)) {
      status.setData(s);
    } else {
      void status.reload();
    }
  });

  return (
    <Card title={t('tunnel.title')} description={t('tunnel.desc')}>
      <div className="mb-3 space-y-2 rounded-md bg-warning-bg p-3 text-sm text-warning" role="note">
        <p>⚠ {t('tunnel.quickNote')}</p>
        <p>{t('tunnel.accessNote')}</p>
      </div>
      <AsyncView state={status}>
        {(s) => (
          <div className="flex flex-col gap-4 text-sm">
            <div className="flex flex-wrap items-center gap-2">
              <StatusPill status={s.running ? 'running' : 'stopped'} label={s.running ? t('tunnel.running') : t('tunnel.stopped')} />
              {s.mode && <Badge>{t(`tunnel.mode.${s.mode}` as TKey)}</Badge>}
              {!s.cloudflaredFound && <Badge tone="warning">{t('tunnel.noCloudflared')}</Badge>}
            </div>
            <KeyValue
              items={[
                [t('tunnel.publicUrl'), s.publicUrl ? <ExternalLink href={s.publicUrl}>{s.publicUrl}</ExternalLink> : '—'],
                [t('tunnel.hostname'), s.hostname ?? '—'],
                ['cloudflared', s.cloudflaredPath ? <Code>{s.cloudflaredPath}</Code> : '—'],
                [t('server.startedAt'), formatDate(s.startedAt, lang)],
                [t('tunnel.token'), s.hasToken ? t('tunnel.tokenSaved') : t('tunnel.tokenNone')],
              ]}
            />
            {s.lastError && (
              <p className="text-danger" role="alert">
                {s.lastError}
              </p>
            )}
            {!s.cloudflaredFound && <p className="text-muted">{t('tunnel.installHint')}</p>}
            <TunnelConfigForm status={s} onSaved={(n) => (n ? status.setData(n) : void status.reload())} onDeleteToken={() => setConfirm('deleteToken')} />
            <div className="flex flex-wrap gap-2">
              <HostOnly>
                {!s.running ? (
                  <Button variant="primary" size="sm" disabled={!s.cloudflaredFound} onClick={() => setConfirm('start')}>
                    {t('tunnel.start')}
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="danger"
                    loading={busy === 'stop'}
                    onClick={async () => {
                      setBusy('stop');
                      try {
                        const r = await api.tunnel.stop();
                        if (r && typeof r === 'object' && 'running' in r) status.setData(r);
                        else await status.reload();
                      } catch (e) {
                        notify.fail(e);
                      } finally {
                        setBusy(null);
                      }
                    }}
                  >
                    {t('tunnel.stop')}
                  </Button>
                )}
              </HostOnly>
            </div>
            <div>
              <p className="mb-1 font-semibold">{t('tunnel.log')}</p>
              <LogBox lines={s.logTail} label={t('tunnel.log')} />
            </div>
            <ConfirmDialog
              open={confirm === 'start'}
              tone="danger"
              title={t('tunnel.startTitle')}
              confirmLabel={t('tunnel.start')}
              cancelLabel={t('common.cancel')}
              acknowledge={t('tunnel.ack')}
              onCancel={() => setConfirm(null)}
              onConfirm={async () => {
                const r = await api.tunnel.start();
                if (r && typeof r === 'object' && 'running' in r) status.setData(r);
                else await status.reload();
                setConfirm(null);
              }}
            >
              <ul className="list-disc space-y-1 pl-5">
                <li>{t('tunnel.risk1')}</li>
                <li>{t('tunnel.risk2')}</li>
                <li>{t('tunnel.quickNote')}</li>
              </ul>
            </ConfirmDialog>
            <ConfirmDialog
              open={confirm === 'deleteToken'}
              tone="danger"
              title={t('tunnel.deleteToken')}
              confirmLabel={t('common.delete')}
              cancelLabel={t('common.cancel')}
              onCancel={() => setConfirm(null)}
              onConfirm={async () => {
                await api.tunnel.deleteToken();
                await status.reload();
                setConfirm(null);
              }}
            >
              <p>{t('tunnel.deleteTokenBody')}</p>
            </ConfirmDialog>
          </div>
        )}
      </AsyncView>
    </Card>
  );
}

function TunnelConfigForm({
  status,
  onSaved,
  onDeleteToken,
}: {
  status: TunnelStatus;
  onSaved: (s: TunnelStatus | undefined) => void;
  onDeleteToken: () => void;
}) {
  const { t } = useI18n();
  const notify = useNotify();
  const { hostControl } = useCaps();
  const [mode, setMode] = useState<TunnelMode>(status.mode ?? 'quick');
  const [hostname, setHostname] = useState(status.hostname ?? '');
  const [token, setToken] = useState('');
  const [autoStart, setAutoStart] = useState(status.autoStart);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | undefined>();
  useEffect(() => {
    setAutoStart(status.autoStart);
  }, [status.autoStart]);

  return (
    <form
      className="flex flex-col gap-3 rounded-md border border-line p-3"
      onSubmit={async (e) => {
        e.preventDefault();
        if (mode === 'named' && !hostname.trim()) {
          setErr(t('tunnel.hostnameRequired'));
          return;
        }
        setErr(undefined);
        setBusy(true);
        try {
          const r = await api.tunnel.putConfig({
            mode,
            hostname: mode === 'named' ? hostname.trim() : null,
            autoStart,
            ...(token ? { token } : {}),
          });
          setToken('');
          onSaved(r && typeof r === 'object' && 'running' in r ? r : undefined);
          notify.ok(t('common.saved'));
        } catch (e2) {
          notify.fail(e2);
        } finally {
          setBusy(false);
        }
      }}
    >
      <fieldset disabled={!hostControl} className="grid gap-3 sm:grid-cols-2">
        <Select
          label={t('tunnel.modeLabel')}
          value={mode}
          onChange={(e) => setMode(e.target.value as TunnelMode)}
          options={[
            { value: 'quick', label: t('tunnel.mode.quick') },
            { value: 'named', label: t('tunnel.mode.named') },
          ]}
        />
        {mode === 'named' && (
          <TextInput label={t('tunnel.hostname')} placeholder="video.example.com" value={hostname} onChange={(e) => setHostname(e.target.value)} error={err} />
        )}
        {mode === 'named' && (
          <TextInput
            label={t('tunnel.token')}
            type="password"
            autoComplete="off"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder={status.hasToken ? t('tunnel.tokenPlaceholderSaved') : ''}
            hint={t('tunnel.tokenHint')}
          />
        )}
        <Checkbox className="sm:col-span-2" label={t('tunnel.autoStart')} checked={autoStart} onChange={(e) => setAutoStart(e.target.checked)} />
      </fieldset>
      <div className="flex flex-wrap gap-2">
        <HostOnly>
          <Button type="submit" size="sm" variant="primary" loading={busy}>
            {t('tunnel.saveConfig')}
          </Button>
          {status.hasToken && (
            <Button size="sm" variant="ghost" onClick={onDeleteToken}>
              {t('tunnel.deleteToken')}
            </Button>
          )}
        </HostOnly>
      </div>
    </form>
  );
}

export function MultiTunnelsCard() {
  const { t } = useI18n();
  const notify = useNotify();
  const overview = useApi('tunnels-overview', () => api.tunnels.overview());
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [kind, setKind] = useState<ManagedTunnelKind>('quick');
  const [targetUrl, setTargetUrl] = useState('');
  const [hostname, setHostname] = useState('');
  const [cfName, setCfName] = useState('');
  const [token, setToken] = useState('');
  const [autoStart, setAutoStart] = useState(false);
  const [notes, setNotes] = useState('');
  const [confirmQuick, setConfirmQuick] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  useServerEvent('tunnel', () => {
    void overview.reload();
  });

  return (
    <Card
      title={t('tunnels.crudTitle')}
      description={t('tunnels.crudDesc')}
      actions={
        <HostOnly>
          <Button size="sm" onClick={() => void overview.reload()}>
            {t('common.reload')}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => setCreating((v) => !v)}>
            + {t('tunnels.addBtn')}
          </Button>
          <Button size="sm" variant="primary" disabled={!overview.data?.cloudflaredFound} onClick={() => setConfirmQuick(true)}>
            ⚡ {t('tunnels.quickWebBtn')}
          </Button>
        </HostOnly>
      }
    >
      <AsyncView state={overview}>
        {(ov) => (
          <div className="flex flex-col gap-4 text-sm">
            {creating && (
              <form
                className="grid gap-3 rounded-md border border-line bg-surface-2/40 p-3 sm:grid-cols-2"
                onSubmit={async (e) => {
                  e.preventDefault();
                  const req: ManagedTunnelReq = {
                    name: name.trim(),
                    kind,
                    targetUrl: targetUrl.trim() || ov.defaultTargetUrl,
                    hostname: hostname.trim() || null,
                    cfTunnelName: cfName.trim() || null,
                    autoStart,
                    notes: notes.trim(),
                    ...(token.trim() ? { token: token.trim() } : {}),
                  };
                  try {
                    await api.tunnels.create(req);
                    setCreating(false);
                    setName('');
                    setToken('');
                    notify.ok(t('common.saved'));
                    await overview.reload();
                  } catch (ex) {
                    notify.fail(ex);
                  }
                }}
              >
                <TextInput label={t('tunnels.name')} placeholder="Prod Studio" value={name} onChange={(e) => setName(e.target.value)} />
                <Select
                  label={t('tunnel.modeLabel')}
                  value={kind}
                  onChange={(e) => setKind(e.target.value as ManagedTunnelKind)}
                  options={[
                    { value: 'quick', label: t('tunnels.kind.quick') },
                    { value: 'token', label: t('tunnels.kind.token') },
                    { value: 'named', label: t('tunnels.kind.named') },
                  ]}
                />
                <TextInput
                  label={t('tunnels.targetUrl')}
                  placeholder={ov.defaultTargetUrl}
                  value={targetUrl}
                  onChange={(e) => setTargetUrl(e.target.value)}
                />
                <TextInput label={t('tunnel.hostname')} placeholder="studio.example.com" value={hostname} onChange={(e) => setHostname(e.target.value)} />
                {kind === 'token' && (
                  <TextInput
                    label={t('tunnel.token')}
                    type="password"
                    autoComplete="off"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    hint={t('tunnel.tokenHint')}
                  />
                )}
                {kind === 'named' && (
                  <TextInput label={t('tunnels.cfName')} placeholder="my-tunnel" value={cfName} onChange={(e) => setCfName(e.target.value)} />
                )}
                <TextInput label={t('tunnels.notes')} value={notes} onChange={(e) => setNotes(e.target.value)} />
                <Checkbox className="sm:col-span-2" label={t('tunnel.autoStart')} checked={autoStart} onChange={(e) => setAutoStart(e.target.checked)} />
                <div className="sm:col-span-2 flex gap-2">
                  <Button type="submit" size="sm" variant="primary" disabled={!name.trim()}>
                    {t('common.save')}
                  </Button>
                  <Button type="button" size="sm" variant="ghost" onClick={() => setCreating(false)}>
                    {t('common.cancel')}
                  </Button>
                </div>
              </form>
            )}

            {ov.managed.length === 0 ? (
              <p className="text-muted">{t('tunnels.empty')}</p>
            ) : (
              <ul className="flex flex-col gap-2" data-testid="managed-tunnels-list">
                {ov.managed.map((m: ManagedTunnel) => (
                  <li key={m.id} className="rounded-md border border-line p-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <strong>{m.name}</strong>
                        <Badge tone="info">{t(`tunnels.kind.${m.kind}` as TKey)}</Badge>
                        <StatusPill status={m.running ? 'running' : 'stopped'} label={m.running ? t('tunnel.running') : t('tunnel.stopped')} />
                        {m.hasToken && <Badge tone="success">{t('tunnel.tokenSaved')}</Badge>}
                      </div>
                      <div className="flex flex-wrap gap-1.5">
                        {!m.running ? (
                          <Button
                            size="sm"
                            variant="primary"
                            disabled={!ov.cloudflaredFound}
                            loading={busyId === m.id}
                            onClick={async () => {
                              setBusyId(m.id);
                              try {
                                await api.tunnels.start(m.id);
                                await overview.reload();
                              } catch (ex) {
                                notify.fail(ex);
                              } finally {
                                setBusyId(null);
                              }
                            }}
                          >
                            {t('tunnel.start')}
                          </Button>
                        ) : (
                          <Button
                            size="sm"
                            variant="danger"
                            loading={busyId === m.id}
                            onClick={async () => {
                              setBusyId(m.id);
                              try {
                                await api.tunnels.stop(m.id);
                                await overview.reload();
                              } catch (ex) {
                                notify.fail(ex);
                              } finally {
                                setBusyId(null);
                              }
                            }}
                          >
                            {t('tunnel.stop')}
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={async () => {
                            try {
                              await api.tunnels.remove(m.id);
                              await overview.reload();
                            } catch (ex) {
                              notify.fail(ex);
                            }
                          }}
                        >
                          {t('common.delete')}
                        </Button>
                      </div>
                    </div>
                    <p className="mt-1 text-xs text-muted">
                      <Code>{m.targetUrl}</Code>
                      {m.publicUrl && (
                        <>
                          {' → '}
                          <ExternalLink href={m.publicUrl}>{m.publicUrl}</ExternalLink>
                        </>
                      )}
                    </p>
                    {m.lastError && <p className="mt-1 text-xs text-danger">{m.lastError}</p>}
                    {m.logTail.length > 0 && (
                      <details className="mt-2">
                        <summary className="cursor-pointer text-xs text-muted">{t('tunnel.log')}</summary>
                        <LogBox lines={m.logTail} label={m.name} />
                      </details>
                    )}
                  </li>
                ))}
              </ul>
            )}

            {ov.external.length > 0 && (
              <div className="rounded-md border border-line p-3">
                <p className="mb-2 font-semibold">{t('tunnels.externalTitle', { n: ov.external.length })}</p>
                <ul className="divide-y divide-line text-xs">
                  {ov.external.map((p) => (
                    <li key={p.pid} className="flex flex-wrap items-center justify-between gap-2 py-1.5">
                      <span>
                        <strong>PID {p.pid}</strong> · {p.targetHint} · <Code>{p.commandLine}</Code>
                      </span>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={async () => {
                          try {
                            await api.tunnels.stopExternal(p.pid);
                            await overview.reload();
                          } catch (ex) {
                            notify.fail(ex);
                          }
                        }}
                      >
                        {t('server.stop')}
                      </Button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
      </AsyncView>
      <ConfirmDialog
        open={confirmQuick}
        tone="danger"
        title={t('tunnel.startTitle')}
        confirmLabel={t('tunnels.quickWebBtn')}
        cancelLabel={t('common.cancel')}
        acknowledge={t('tunnel.ack')}
        onCancel={() => setConfirmQuick(false)}
        onConfirm={async () => {
          await api.tunnels.quickWeb();
          setConfirmQuick(false);
          await overview.reload();
        }}
      >
        <p>{t('tunnel.quickNote')}</p>
      </ConfirmDialog>
    </Card>
  );
}
