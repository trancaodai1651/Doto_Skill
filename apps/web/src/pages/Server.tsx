import { useEffect, useState } from 'react';
import { Badge, Button, Card, Checkbox, ConfirmDialog, Select, TextArea, TextInput, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { ServerConfig, ServerMode, ServerStatus, TunnelMode, TunnelStatus } from '../api/types';
import { AsyncView, Code, ExternalLink, HostOnly, KeyValue, LogBox, PageHeader, QrImage, useNotify } from '../components/common';
import { FirewallGuide, LanConfirmDialog } from '../components/features';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';
import { useCaps } from '../state/session';

export function ServerPage() {
  const { t } = useI18n();
  return (
    <>
      <PageHeader title={t('nav.server')} description={t('server.desc')} />
      <div className="flex flex-col gap-4">
        <ServerStatusCard />
        <ServerConfigCard />
        <TunnelCard />
      </div>
    </>
  );
}

function ServerStatusCard() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const { isDesktopWindow } = useCaps();
  const status = useApi('server-status', () => api.server.status());
  const guide = useApi('fw', () => api.server.firewallGuide());
  const [busy, setBusy] = useState<string | null>(null);
  const [confirmStop, setConfirmStop] = useState(false);
  const [qrFor, setQrFor] = useState<string | null>(null);
  useServerEvent('server', (s) => status.setData(s));

  async function act(kind: 'start' | 'stop' | 'restart') {
    setBusy(kind);
    try {
      const r = await api.server[kind]();
      if (r && typeof r === 'object' && 'running' in r) status.setData(r);
      else await status.reload();
    } catch (e) {
      notify.fail(e);
      throw e;
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card title={t('server.status')}>
      <AsyncView state={status}>
        {(s) => (
          <div className="flex flex-col gap-3 text-sm">
            <div className="flex flex-wrap items-center gap-2">
              <StatusPill status={s.running ? 'running' : 'stopped'} label={s.running ? t('server.running') : t('server.stopped')} />
              <Badge>{t(`server.mode.${s.mode}` as TKey)}</Badge>
            </div>
            <KeyValue
              items={[
                [t('server.bind'), s.bindAddress ? <Code>{`${s.bindAddress}:${s.port}`}</Code> : '—'],
                [t('server.connectionsLabel'), String(s.activeConnections)],
                [t('server.startedAt'), formatDate(s.startedAt, lang)],
                ['PID', String(s.pid)],
              ]}
            />
            {s.lastError && (
              <p className="text-danger" role="alert">
                {s.lastError}
              </p>
            )}
            {s.urls.length > 0 && (
              <div>
                <p className="mb-1 font-semibold">{t('server.urls')}</p>
                <ul className="flex flex-col gap-2">
                  {s.urls.map((u) => (
                    <li key={u} className="flex flex-wrap items-center gap-2">
                      <ExternalLink href={u}>{u}</ExternalLink>
                      <Button size="sm" variant="ghost" onClick={() => setQrFor(qrFor === u ? null : u)} aria-expanded={qrFor === u}>
                        QR
                      </Button>
                      {qrFor === u && (
                        <div className="w-full">
                          <QrImage src={api.server.qrUrl(u)} alt={t('server.qrAlt', { url: u })} />
                        </div>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            <div className="flex flex-wrap gap-2">
              <HostOnly>
                {!s.running && (
                  <Button variant="primary" size="sm" loading={busy === 'start'} onClick={() => void act('start').catch(() => undefined)}>
                    {t('server.start')}
                  </Button>
                )}
                {s.running && (
                  <>
                    <Button size="sm" loading={busy === 'restart'} onClick={() => void act('restart').catch(() => undefined)}>
                      {t('server.restart')}
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => setConfirmStop(true)}>
                      {t('server.stop')}
                    </Button>
                  </>
                )}
              </HostOnly>
            </div>
            <FirewallGuide state={guide} />
            <ConfirmDialog
              open={confirmStop}
              tone="danger"
              title={t('server.stopTitle')}
              confirmLabel={t('server.stop')}
              cancelLabel={t('common.cancel')}
              onCancel={() => setConfirmStop(false)}
              onConfirm={async () => {
                await act('stop');
                setConfirmStop(false);
              }}
            >
              <p>{t('server.stopBody')}</p>
              {!isDesktopWindow && (
                <p className="mt-2 rounded-md bg-warning-bg p-2 text-sm text-warning" role="note">
                  ⚠ {t('server.stopRemoteWarn')}
                </p>
              )}
            </ConfirmDialog>
          </div>
        )}
      </AsyncView>
    </Card>
  );
}

function ServerConfigCard() {
  const { t } = useI18n();
  const cfg = useApi('server-config', () => api.server.config());
  const status = useApi('server-status-cfg', () => api.server.status());
  return (
    <Card title={t('server.config')} description={t('server.configDesc')}>
      <AsyncView state={cfg}>
        {(c) => <ServerConfigForm key={JSON.stringify(c)} config={c} lan={status.data?.lanAddresses ?? []} onSaved={(n) => cfg.setData(n)} />}
      </AsyncView>
    </Card>
  );
}

function ServerConfigForm({ config, lan, onSaved }: { config: ServerConfig; lan: ServerStatus['lanAddresses']; onSaved: (c: ServerConfig) => void }) {
  const { t } = useI18n();
  const notify = useNotify();
  const { hostControl } = useCaps();
  const [mode, setMode] = useState<ServerMode>(config.mode);
  const [port, setPort] = useState(String(config.port));
  const [iface, setIface] = useState(config.lanInterfaceIp ?? '');
  const [allow, setAllow] = useState(config.ipAllowlist.join('\n'));
  const [publicBaseUrl, setPublicBaseUrl] = useState(config.publicBaseUrl ?? '');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [confirm, setConfirm] = useState<'lan' | 'save' | null>(null);

  function build(): ServerConfig | null {
    const errs: Record<string, string> = {};
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1024 || p > 65535) errs.port = t('server.portRange');
    const list = allow
      .split(/[\n,]/)
      .map((x) => x.trim())
      .filter(Boolean);
    const bad = list.find((x) => !/^[0-9a-fA-F:.]+(\/\d{1,3})?$/.test(x));
    if (bad) errs.allow = t('server.badCidr', { value: bad });
    if (publicBaseUrl.trim() && !/^https?:\/\/[^\s/]+/i.test(publicBaseUrl.trim())) errs.publicBaseUrl = t('server.badUrl');
    setErrors(errs);
    if (Object.keys(errs).length) return null;
    return { mode, port: p, lanInterfaceIp: iface || null, ipAllowlist: list, publicBaseUrl: publicBaseUrl.trim() || null };
  }

  async function save(next: ServerConfig) {
    const r = await api.server.putConfig(next);
    onSaved(r && typeof r === 'object' && 'mode' in r ? r : next);
    notify.ok(t('common.saved'));
  }

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        const next = build();
        if (!next) return;
        setConfirm(next.mode === 'lan' && config.mode !== 'lan' ? 'lan' : 'save');
      }}
    >
      <fieldset disabled={!hostControl} className="grid gap-3 sm:grid-cols-2">
        <Select
          label={t('server.mode')}
          value={mode}
          onChange={(e) => setMode(e.target.value as ServerMode)}
          options={(['off', 'localhost', 'lan'] as ServerMode[]).map((m) => ({ value: m, label: t(`server.mode.${m}` as TKey) }))}
        />
        <TextInput label={t('server.port')} inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value)} error={errors.port} />
        {mode === 'lan' && (
          <Select
            label={t('server.interface')}
            value={iface}
            onChange={(e) => setIface(e.target.value)}
            options={[
              { value: '', label: t('server.allInterfaces') },
              ...lan.map((l) => ({ value: l.ip, label: `${l.interface} · ${l.ip}${l.scope === 'public' ? ' ⚠ IP public' : l.scope === 'cgnat' ? ' (VPN/CGNAT)' : ''}` })),
            ]}
          />
        )}
        {mode === 'lan' && lan.some((l) => l.scope === 'public') && (!iface || lan.find((l) => l.ip === iface)?.scope === 'public') && (
          <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 p-2 text-sm text-danger">
            ⚠ Máy này có địa chỉ IP public. Lắng nghe trên “tất cả interface” hoặc IP public sẽ mở máy chủ ra Internet — hãy chọn interface mạng riêng/VPN cụ thể, đặt IP allowlist, hoặc dùng Cloudflare Tunnel.
          </p>
        )}
        <TextInput
          label={t('server.publicBaseUrl')}
          placeholder="https://video.example.com"
          value={publicBaseUrl}
          onChange={(e) => setPublicBaseUrl(e.target.value)}
          error={errors.publicBaseUrl}
          hint={t('server.publicBaseUrlHint')}
          inputMode="url"
        />
        <TextArea
          fieldClassName="sm:col-span-2"
          label={t('server.allowlist')}
          hint={t('server.allowlistHint')}
          value={allow}
          onChange={(e) => setAllow(e.target.value)}
          error={errors.allow}
          rows={3}
          spellCheck={false}
        />
      </fieldset>
      <div>
        <HostOnly>
          <Button type="submit" variant="primary">
            {t('common.save')}
          </Button>
        </HostOnly>
      </div>
      <LanConfirmDialog
        open={confirm === 'lan'}
        onCancel={() => setConfirm(null)}
        onConfirm={async () => {
          const next = build();
          if (next) await save(next);
          setConfirm(null);
        }}
      />
      <ConfirmDialog
        open={confirm === 'save'}
        title={t('server.saveTitle')}
        confirmLabel={t('common.save')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirm(null)}
        onConfirm={async () => {
          const next = build();
          if (next) await save(next);
          setConfirm(null);
        }}
      >
        <p>{t('server.saveBody')}</p>
      </ConfirmDialog>
    </form>
  );
}

function TunnelCard() {
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
