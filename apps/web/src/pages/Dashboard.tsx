import { Link } from 'react-router';
import { Card, EmptyState, Progress, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useEvents, useServerEvent } from '../api/events';
import type { Job } from '../api/types';
import { AsyncView, ExternalLink, KeyValue, PageHeader, Sparkline } from '../components/common';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { formatEta, formatMb, formatPercent } from '../lib/format';
import { useApi } from '../lib/useApi';
import { upsertById } from '../lib/list';
import { useSession } from '../state/session';

export function DashboardPage() {
  const { t, lang } = useI18n();
  const { me } = useSession();
  const hw = useApi('hw', () => api.system.hardware());
  const metricsInit = useApi('metrics', () => api.system.metrics());
  const server = useApi('server', () => api.server.status());
  const jobs = useApi('jobs', () => api.jobs.list());
  const { latestMetrics, metricsHistory } = useEvents();

  useServerEvent('server', (s) => server.setData(s));
  useServerEvent('job', (j) => jobs.setData((l) => upsertById(l, j)));

  const m = latestMetrics ?? metricsInit.data;
  const active = (jobs.data ?? []).filter((j: Job) => j.status === 'running' || j.status === 'queued');

  return (
    <>
      <PageHeader title={t('nav.dashboard')} description={t('dash.desc', { version: me?.version ?? '' })} />
      <div className="grid gap-4 lg:grid-cols-2">
        <Card title={t('dash.metrics')}>
          {m ? (
            <div className="grid gap-4 sm:grid-cols-2">
              <div>
                <p className="text-sm text-muted">CPU</p>
                <p className="text-2xl font-bold" aria-live="polite">
                  {formatPercent(m.cpuPercent)}
                </p>
                <Sparkline values={metricsHistory.map((x) => x.cpuPercent)} max={100} label={t('perf.cpuChart')} />
              </div>
              <div>
                <p className="text-sm text-muted">RAM</p>
                <p className="text-2xl font-bold">
                  {formatMb(m.memUsedMb)} <span className="text-sm font-normal text-muted">/ {formatMb(m.memTotalMb)}</span>
                </p>
                <Sparkline
                  values={metricsHistory.map((x) => x.memUsedMb)}
                  max={m.memTotalMb}
                  label={t('perf.ramChart')}
                  color="var(--doto-success)"
                />
              </div>
              <p className="text-sm sm:col-span-2">
                {t('dash.jobsCount', { running: m.runningJobs, queued: m.queuedJobs, max: m.limits.maxJobs })} · {t('profile.label')}:{' '}
                {t(`profile.${m.profile}` as TKey)}
                {m.throttleReason && <span className="mt-1 block text-warning">⚠ {m.throttleReason}</span>}
              </p>
            </div>
          ) : (
            <AsyncView state={metricsInit}>{() => null}</AsyncView>
          )}
        </Card>

        <Card title={t('dash.server')} actions={<Link to="/server" className="text-sm text-primary underline">{t('common.manage')}</Link>}>
          <AsyncView state={server}>
            {(s) => (
              <div className="flex flex-col gap-2 text-sm">
                <div className="flex flex-wrap items-center gap-2">
                  <StatusPill status={s.running ? 'running' : 'stopped'} label={s.running ? t('server.running') : t('server.stopped')} />
                  <span className="text-muted">{t(`server.mode.${s.mode}` as TKey)}</span>
                  {s.running && <span className="text-muted">· {t('server.connections', { n: s.activeConnections })}</span>}
                </div>
                {s.urls.length > 0 && (
                  <ul className="flex flex-col gap-1">
                    {s.urls.map((u) => (
                      <li key={u}>
                        <ExternalLink href={u}>{u}</ExternalLink>
                      </li>
                    ))}
                  </ul>
                )}
                {s.lastError && <p className="text-danger">{s.lastError}</p>}
              </div>
            )}
          </AsyncView>
        </Card>

        <Card title={t('dash.host')}>
          <AsyncView state={hw}>
            {(h) => (
              <KeyValue
                items={[
                  [t('hw.host'), h.hostname],
                  [t('hw.os'), `${h.osName} ${h.osVersion} (${h.arch})`],
                  [t('hw.cpu'), `${h.cpuBrand} · ${h.logicalCores} ${t('hw.threads')}`],
                  [t('hw.ram'), formatMb(h.totalMemoryMb)],
                  [t('hw.gpu'), h.gpus.map((g) => g.name).join(', ') || t('hw.noGpu')],
                ]}
              />
            )}
          </AsyncView>
        </Card>

        <Card title={t('dash.activeJobs')} actions={<Link to="/jobs" className="text-sm text-primary underline">{t('common.viewAll')}</Link>}>
          <AsyncView state={jobs}>
            {() =>
              active.length === 0 ? (
                <EmptyState title={t('jobs.noneActive')} />
              ) : (
                <ul className="flex flex-col gap-3">
                  {active.slice(0, 5).map((j) => (
                    <li key={j.id}>
                      <div className="mb-1 flex items-center justify-between gap-2 text-sm">
                        <span className="truncate font-medium">{j.title}</span>
                        <StatusPill status={j.status} label={t(`job.status.${j.status}` as TKey)} />
                      </div>
                      <Progress
                        value={j.status === 'queued' ? null : j.progress}
                        ariaLabel={j.title}
                        detail={j.etaSeconds != null ? `ETA ${formatEta(j.etaSeconds, lang)}` : undefined}
                      />
                    </li>
                  ))}
                </ul>
              )
            }
          </AsyncView>
        </Card>
      </div>
    </>
  );
}
