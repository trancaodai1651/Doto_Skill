import { useState } from 'react';
import type { FormEvent, ReactNode } from 'react';
import { Badge, Button, Checkbox, ConfirmDialog, TextInput, cx } from '@doto/ui';
import { api } from '../api/client';
import type {
  HardwareInfo,
  InstallMethod,
  Job,
  ProfileId,
  ResourceLimits,
  ResourceSettings,
  SkillSource,
  ToolStatus,
} from '../api/types';
import { useI18n, useT } from '../i18n';
import type { TKey } from '../i18n';
import { formatBytes, formatDate, formatMb } from '../lib/format';
import { useApi } from '../lib/useApi';
import type { ApiState } from '../lib/useApi';
import { AsyncView, Code, ExternalLink, HostOnly, KeyValue, useNotify } from './common';

// ---------------------------------------------------------------------------
// AI CLI install (never auto-run: always a ConfirmDialog with the exact command)
// ---------------------------------------------------------------------------
export function InstallMethodButton({
  tool,
  method,
  onStarted,
}: {
  tool: ToolStatus;
  method: InstallMethod;
  onStarted?: (job: Job) => void;
}) {
  const t = useT();
  const notify = useNotify();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button size="sm" variant="secondary" disabled={!method.available} onClick={() => setOpen(true)} title={method.unavailableReason ?? undefined}>
        {t('tools.installWith', { method: method.label })}
      </Button>
      {!method.available && method.unavailableReason && <span className="text-xs text-muted">{method.unavailableReason}</span>}
      <ConfirmDialog
        open={open}
        wide
        title={t('tools.installTitle', { name: tool.name })}
        confirmLabel={t('tools.installConfirm')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setOpen(false)}
        onConfirm={async () => {
          const job = await api.tools.install(tool.id, method.id);
          setOpen(false);
          notify.ok(t('tools.installStarted', { name: tool.name }));
          onStarted?.(job);
        }}
      >
        <p>{t('tools.installIntro')}</p>
        <p className="mt-3 text-sm font-semibold">{t('tools.command')}</p>
        <pre className="mt-1 overflow-x-auto rounded-md bg-surface-2 p-3 font-mono text-xs whitespace-pre-wrap break-all" data-testid="install-command">
          {method.command}
        </pre>
        <p className="mt-3 text-sm">
          {t('tools.source')}: <ExternalLink href={method.sourceUrl}>{method.sourceUrl}</ExternalLink>
        </p>
        {method.requiresElevation && (
          <p className="mt-3 rounded-md bg-warning-bg p-3 text-sm text-warning" role="note">
            ⚠ {t('tools.elevation')}
          </p>
        )}
      </ConfirmDialog>
    </>
  );
}

export function authTone(s: ToolStatus['authStatus']) {
  return s === 'logged_in' ? 'success' : s === 'logged_out' ? 'warning' : 'neutral';
}

// ---------------------------------------------------------------------------
// Skill sources
// ---------------------------------------------------------------------------
export function HostDiskHints({ onSelect }: { onSelect: (mountPoint: string) => void }) {
  const t = useT();
  const hw = useApi('hw-disks', () => api.system.hardware());
  const disks = hw.data?.disks ?? [];
  if (disks.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted">
      <span>{t('folders.hostDisks')}:</span>
      {disks.map((d) => (
        <button
          key={d.mountPoint}
          type="button"
          onClick={() => onSelect(d.mountPoint)}
          className="rounded border border-line bg-surface-2 px-2 py-0.5 font-mono text-fg hover:border-primary"
          title={`${d.mountPoint} · ${formatBytes(d.availableBytes)}`}
        >
          {d.mountPoint} ({formatBytes(d.availableBytes)})
        </button>
      ))}
    </div>
  );
}

export function AddFolderSourceForm({ onAdded }: { onAdded?: (s: SkillSource) => void }) {
  const t = useT();
  const notify = useNotify();
  const [path, setPath] = useState('');
  const [watch, setWatch] = useState(true);
  const [err, setErr] = useState<string | undefined>();
  const [confirm, setConfirm] = useState(false);

  function submit(e: FormEvent) {
    e.preventDefault();
    if (!isAbsolutePath(path.trim())) {
      setErr(t('path.absolute'));
      return;
    }
    setErr(undefined);
    setConfirm(true);
  }
  return (
    <HostOnly>
      <form onSubmit={submit} className="flex flex-col gap-2" aria-label={t('sources.addFolder')}>
        <TextInput
          label={t('sources.folderPath')}
          placeholder={t('sources.folderPlaceholder')}
          value={path}
          onChange={(e) => setPath(e.target.value)}
          error={err}
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
        />
        <HostDiskHints onSelect={(m) => setPath((cur) => cur || m)} />
        <Checkbox label={t('sources.watch')} checked={watch} onChange={(e) => setWatch(e.target.checked)} />
        <div>
          <Button type="submit" variant="primary">
            {t('sources.addFolder')}
          </Button>
        </div>
      </form>
      <ConfirmDialog
        open={confirm}
        title={t('sources.addFolderConfirmTitle')}
        confirmLabel={t('sources.addFolder')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirm(false)}
        onConfirm={async () => {
          const s = await api.skills.addFolder(path.trim(), watch);
          setConfirm(false);
          setPath('');
          notify.ok(t('sources.added'));
          onAdded?.(s);
        }}
      >
        <p>{t('sources.addFolderConfirmBody')}</p>
        <p className="mt-2">
          <Code>{path.trim()}</Code>
        </p>
      </ConfirmDialog>
    </HostOnly>
  );
}

export function AddGithubSourceForm({ onAdded }: { onAdded?: (s: SkillSource) => void }) {
  const t = useT();
  const notify = useNotify();
  const [url, setUrl] = useState('');
  const [ref, setRef] = useState('');
  const [isPrivate, setPrivate] = useState(false);
  const [err, setErr] = useState<string | undefined>();
  const [confirm, setConfirm] = useState(false);
  function submit(e: FormEvent) {
    e.preventDefault();
    if (!/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+/i.test(url.trim())) {
      setErr(t('sources.githubUrlRule'));
      return;
    }
    setErr(undefined);
    setConfirm(true);
  }
  return (
    <HostOnly>
      <form onSubmit={submit} className="flex flex-col gap-2" aria-label={t('sources.addGithub')}>
        <TextInput
          label={t('sources.githubUrl')}
          placeholder="https://github.com/owner/repo"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          error={err}
          inputMode="url"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
        />
        <TextInput label={t('sources.githubRef')} placeholder="main" value={ref} onChange={(e) => setRef(e.target.value)} hint={t('sources.githubRefHint')} />
        <Checkbox label={t('sources.private')} checked={isPrivate} onChange={(e) => setPrivate(e.target.checked)} />
        <div>
          <Button type="submit" variant="primary">
            {t('sources.addGithub')}
          </Button>
        </div>
      </form>
      <ConfirmDialog
        open={confirm}
        title={t('sources.addGithubConfirmTitle')}
        confirmLabel={t('sources.addGithub')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirm(false)}
        onConfirm={async () => {
          const s = await api.skills.addGithub(url.trim(), ref.trim() || undefined, isPrivate);
          setConfirm(false);
          setUrl('');
          setRef('');
          notify.ok(t('sources.added'));
          onAdded?.(s);
        }}
      >
        <p>{t('sources.addGithubConfirmBody')}</p>
        <p className="mt-2">
          <Code>{url.trim()}</Code> {ref.trim() && <Code>@{ref.trim()}</Code>}
        </p>
      </ConfirmDialog>
    </HostOnly>
  );
}

export function isAbsolutePath(p: string): boolean {
  return /^([A-Za-z]:[\\/]|\\\\|\/)/.test(p);
}

// ---------------------------------------------------------------------------
// Server LAN exposure confirmation
// ---------------------------------------------------------------------------
export function LanConfirmDialog({ open, onCancel, onConfirm }: { open: boolean; onCancel: () => void; onConfirm: () => Promise<void> }) {
  const t = useT();
  const guide = useApi(open ? 'fw' : null, () => api.server.firewallGuide());
  return (
    <ConfirmDialog
      open={open}
      wide
      tone="danger"
      title={t('lan.title')}
      confirmLabel={t('lan.confirm')}
      cancelLabel={t('common.cancel')}
      acknowledge={t('lan.ack')}
      onCancel={onCancel}
      onConfirm={onConfirm}
    >
      <ul className="list-disc space-y-1 pl-5">
        <li>{t('lan.risk1')}</li>
        <li>{t('lan.risk2')}</li>
        <li>{t('lan.risk3')}</li>
      </ul>
      <FirewallGuide state={guide} />
    </ConfirmDialog>
  );
}

export function FirewallGuide({ state }: { state: ApiState<{ os: string; steps: string[]; commands: string[] }> }) {
  const t = useT();
  return (
    <details className="mt-3 rounded-md border border-line p-3">
      <summary className="cursor-pointer text-sm font-semibold">{t('lan.firewall')}</summary>
      <div className="mt-2 text-sm">
        <AsyncView state={state}>
          {(g) => (
            <>
              <p className="text-muted">{g.os}</p>
              <ol className="mt-1 list-decimal space-y-1 pl-5">
                {g.steps.map((s, i) => (
                  <li key={i}>{s}</li>
                ))}
              </ol>
              {g.commands.length > 0 && (
                <pre className="mt-2 overflow-x-auto rounded bg-surface-2 p-2 font-mono text-xs whitespace-pre-wrap">{g.commands.join('\n')}</pre>
              )}
            </>
          )}
        </AsyncView>
      </div>
    </details>
  );
}

// ---------------------------------------------------------------------------
// Resource profile
// ---------------------------------------------------------------------------
export const PROFILES: ProfileId[] = ['eco', 'balanced', 'max'];

export function LimitsTable({ limits, actual }: { limits: ResourceLimits; actual?: Partial<Record<keyof ResourceLimits, ReactNode>> }) {
  const t = useT();
  const rows: Array<[TKey, keyof ResourceLimits, (v: number) => string]> = [
    ['limits.maxJobs', 'maxJobs', String],
    ['limits.maxCpuThreads', 'maxCpuThreads', String],
    ['limits.maxRamMb', 'maxRamMb', formatMb],
    ['limits.maxGpuJobs', 'maxGpuJobs', String],
    ['limits.ffmpegThreads', 'ffmpegThreads', (v) => (v === 0 ? t('limits.auto') : String(v))],
    ['limits.cpuAdmit', 'cpuAdmitThreshold', (v) => `${v}%`],
    ['limits.minFreeRam', 'minFreeRamMb', formatMb],
    ['limits.metricsInterval', 'metricsIntervalMs', (v) => `${v} ms`],
  ];
  return (
    <table className="data" data-testid="limits-table">
      <thead>
        <tr>
          <th>{t('limits.name')}</th>
          <th>{t('limits.effective')}</th>
          {actual && <th>{t('limits.actual')}</th>}
        </tr>
      </thead>
      <tbody>
        {rows.map(([label, k, fmt]) => (
          <tr key={k}>
            <td>{t(label)}</td>
            <td>{fmt(limits[k])}</td>
            {actual && <td>{actual[k] ?? '—'}</td>}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function ProfilePicker({ value, onSelect, busy }: { value: ProfileId; onSelect: (p: ProfileId) => void; busy?: boolean }) {
  const t = useT();
  return (
    <div role="radiogroup" aria-label={t('profile.label')} className="grid gap-3 sm:grid-cols-3">
      {PROFILES.map((p) => {
        const selected = p === value;
        return (
          <button
            key={p}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={busy}
            onClick={() => onSelect(p)}
            className={cx(
              'rounded-lg border p-4 text-left transition-colors',
              selected ? 'border-primary bg-info-bg' : 'border-line bg-surface hover:bg-surface-2',
            )}
          >
            <span className="flex items-center justify-between gap-2 font-semibold">
              {t(`profile.${p}` as TKey)}
              {p === 'balanced' && <Badge tone="info">{t('profile.default')}</Badge>}
            </span>
            <span className="mt-1 block text-sm text-muted">{t(`profile.${p}.desc` as TKey)}</span>
          </button>
        );
      })}
    </div>
  );
}

export function useResourceSettings() {
  return useApi<ResourceSettings>('resources', () => api.settings.resources());
}

// ---------------------------------------------------------------------------
// Hardware
// ---------------------------------------------------------------------------
export function HardwareView({ hw }: { hw: HardwareInfo }) {
  const { t, lang } = useI18n();
  return (
    <div className="flex flex-col gap-4">
      <KeyValue
        items={[
          [t('hw.os'), `${hw.osName} ${hw.osVersion} (${hw.arch})`],
          [t('hw.host'), hw.hostname],
          [t('hw.cpu'), `${hw.cpuBrand} · ${hw.physicalCores ?? '?'} ${t('hw.cores')} / ${hw.logicalCores} ${t('hw.threads')}`],
          [t('hw.ram'), formatMb(hw.totalMemoryMb)],
          [
            t('hw.gpu'),
            hw.gpus.length
              ? hw.gpus.map((g) => `${g.name}${g.vramMb ? ` · ${formatMb(g.vramMb)}` : ''}${g.driverVersion ? ` · driver ${g.driverVersion}` : ''}`).join('; ')
              : t('hw.noGpu'),
          ],
          [
            t('hw.battery'),
            hw.battery?.present
              ? `${hw.battery.percent ?? '?'}%${hw.battery.charging ? ` · ${t('hw.charging')}` : ''}`
              : t('hw.noBattery'),
          ],
          [t('hw.probedAt'), formatDate(hw.probedAt, lang)],
        ]}
      />
      {hw.disks.length > 0 && (
        <div className="overflow-x-auto">
          <table className="data">
            <thead>
              <tr>
                <th>{t('hw.disk')}</th>
                <th>{t('hw.kind')}</th>
                <th>{t('hw.free')}</th>
                <th>{t('hw.total')}</th>
              </tr>
            </thead>
            <tbody>
              {hw.disks.map((d) => (
                <tr key={d.mountPoint}>
                  <td>
                    {d.mountPoint} <span className="text-muted">{d.name}</span>
                  </td>
                  <td>
                    {d.kind.toUpperCase()} · {d.fileSystem}
                  </td>
                  <td>{formatBytes(d.availableBytes)}</td>
                  <td>{formatBytes(d.totalBytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {hw.notes.length > 0 && (
        <ul className="list-disc pl-5 text-sm text-muted">
          {hw.notes.map((n, i) => (
            <li key={i}>{n}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
