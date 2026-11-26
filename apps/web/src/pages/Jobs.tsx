import { useEffect, useState } from 'react';
import { Button, Drawer, EmptyState, Progress, Select, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { Job, JobStatus } from '../api/types';
import { AsyncView, ErrorBlock, KeyValue, LoadingBlock, LogBox, PageHeader, useNotify } from '../components/common';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { formatDate, formatEta } from '../lib/format';
import { upsertById } from '../lib/list';
import { useApi } from '../lib/useApi';

export function JobsPage() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const jobs = useApi('jobs', () => api.jobs.list());
  const [status, setStatus] = useState<'' | JobStatus>('');
  const [openId, setOpenId] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  useServerEvent('job', (j) => jobs.setData((l) => upsertById(l, j)));

  async function act(id: string, kind: 'cancel' | 'retry') {
    setBusy(`${kind}:${id}`);
    try {
      if (kind === 'cancel') await api.jobs.cancel(id);
      else await api.jobs.retry(id);
      await jobs.reload();
    } catch (e) {
      notify.fail(e);
    } finally {
      setBusy(null);
    }
  }

  const statuses: JobStatus[] = ['queued', 'running', 'succeeded', 'failed', 'canceled'];
  return (
    <>
      <PageHeader
        title={t('nav.jobs')}
        description={t('jobs.desc')}
        actions={
          <Button size="sm" onClick={() => void jobs.reload()} loading={jobs.loading}>
            {t('common.reload')}
          </Button>
        }
      />
      <div className="mb-4 max-w-xs">
        <Select
          label={t('jobs.filter')}
          value={status}
          onChange={(e) => setStatus(e.target.value as JobStatus | '')}
          options={[{ value: '', label: t('common.all') }, ...statuses.map((s) => ({ value: s, label: t(`job.status.${s}` as TKey) }))]}
        />
      </div>
      <AsyncView
        state={jobs}
        isEmpty={(l) => l.filter((j) => !status || j.status === status).length === 0}
        empty={<EmptyState icon="≡" title={t('jobs.empty')} description={t('jobs.emptyDesc')} />}
      >
        {(list) => (
          <ul className="flex flex-col gap-3">
            {list
              .filter((j) => !status || j.status === status)
              .map((j) => (
                <li key={j.id} className="rounded-lg border border-line bg-surface p-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <button type="button" className="min-w-0 truncate text-left font-semibold text-primary hover:underline" onClick={() => setOpenId(j.id)}>
                      {j.title}
                    </button>
                    <span className="flex items-center gap-2">
                      <span className="text-xs text-muted">{t(`job.kind.${j.kind}` as TKey)}</span>
                      <StatusPill status={j.status} label={t(`job.status.${j.status}` as TKey)} />
                    </span>
                  </div>
                  {(j.status === 'running' || j.status === 'queued') && (
                    <div className="mt-2">
                      <Progress
                        value={j.status === 'queued' ? null : j.progress}
                        ariaLabel={j.title}
                        detail={[j.etaSeconds != null ? `ETA ${formatEta(j.etaSeconds, lang)}` : null, j.speed, j.encoder].filter(Boolean).join(' · ') || undefined}
                      />
                    </div>
                  )}
                  {j.error && <p className="mt-2 text-sm text-danger">{j.error}</p>}
                  <div className="mt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-muted">
                    <span>
                      {formatDate(j.createdAt, lang)}
                      {j.attempts > 1 && ` · ${t('jobs.attempts', { n: j.attempts })}`}
                      {j.encoder && j.status !== 'running' && ` · ${j.encoder}`}
                    </span>
                    <span className="flex gap-2">
                      {(j.status === 'queued' || j.status === 'running') && (
                        <Button size="sm" loading={busy === `cancel:${j.id}`} onClick={() => act(j.id, 'cancel')}>
                          {t('jobs.cancel')}
                        </Button>
                      )}
                      {(j.status === 'failed' || j.status === 'canceled') && (
                        <Button size="sm" loading={busy === `retry:${j.id}`} onClick={() => act(j.id, 'retry')}>
                          {t('jobs.retry')}
                        </Button>
                      )}
                      <Button size="sm" variant="ghost" onClick={() => setOpenId(j.id)}>
                        {t('jobs.log')}
                      </Button>
                    </span>
                  </div>
                </li>
              ))}
          </ul>
        )}
      </AsyncView>
      <JobDrawer id={openId} onClose={() => setOpenId(null)} />
    </>
  );
}

function JobDrawer({ id, onClose }: { id: string | null; onClose: () => void }) {
  const { t, lang } = useI18n();
  const detail = useApi(id ? `job:${id}` : null, () => api.jobs.get(id!));
  const d = id && detail.data?.id === id ? detail.data : undefined;
  const live = d && (d.status === 'running' || d.status === 'queued');
  const reload = detail.reload;
  useEffect(() => {
    if (!live) return;
    const h = setInterval(() => void reload(), 2000);
    return () => clearInterval(h);
  }, [live, reload]);
  useServerEvent('job', (j: Job) => {
    if (d && j.id === d.id) detail.setData({ ...d, ...j });
  });
  return (
    <Drawer open={id !== null} onClose={onClose} title={d?.title ?? t('jobs.detail')} closeLabel={t('common.close')}>
      {!d ? (
        detail.error ? (
          <ErrorBlock message={detail.error} onRetry={() => void detail.reload()} />
        ) : (
          <LoadingBlock />
        )
      ) : (
        <div className="flex flex-col gap-4">
          <KeyValue
            items={[
              [t('skills.status'), <StatusPill status={d.status} label={t(`job.status.${d.status}` as TKey)} />],
              [t('jobs.kind'), t(`job.kind.${d.kind}` as TKey)],
              [t('jobs.progress'), `${Math.round(d.progress * 100)}%`],
              ['ETA', formatEta(d.etaSeconds, lang)],
              [t('jobs.speed'), d.speed ?? '—'],
              [t('jobs.encoder'), d.encoder ?? '—'],
              [t('jobs.started'), formatDate(d.startedAt, lang)],
              [t('jobs.finished'), formatDate(d.finishedAt, lang)],
            ]}
          />
          {d.error && <p className="text-sm text-danger">{d.error}</p>}
          <LogBox lines={d.log} label={t('jobs.log')} />
        </div>
      )}
    </Drawer>
  );
}
