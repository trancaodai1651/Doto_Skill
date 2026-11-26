import { useState } from 'react';
import { Link } from 'react-router';
import { Badge, Button, Card, EmptyState, Progress, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { Job, ProviderId, ToolStatus } from '../api/types';
import { AsyncView, Code, ExternalLink, HostOnly, KeyValue, LogBox, PageHeader, useNotify } from '../components/common';
import { InstallMethodButton, authTone } from '../components/features';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { formatDate } from '../lib/format';
import { upsertById } from '../lib/list';
import { useApi } from '../lib/useApi';

export function ToolsPage() {
  const { t } = useI18n();
  const tools = useApi('tools', () => api.tools.list());
  const [installJobs, setInstallJobs] = useState<Record<string, Job>>({});
  useServerEvent('tool', (ts) => tools.setData((l) => upsertById(l, ts, false)));
  useServerEvent('job', (j) => {
    setInstallJobs((m) => {
      const entry = Object.entries(m).find(([, job]) => job.id === j.id);
      return entry ? { ...m, [entry[0]]: j } : m;
    });
  });
  return (
    <>
      <PageHeader
        title={t('nav.tools')}
        description={t('tools.desc')}
        actions={
          <Button size="sm" onClick={() => void tools.reload()} loading={tools.loading}>
            {t('common.reload')}
          </Button>
        }
      />
      <AsyncView state={tools} isEmpty={(l) => l.length === 0} empty={<EmptyState title={t('tools.empty')} />}>
        {(list) => (
          <div className="grid gap-4 lg:grid-cols-2">
            {list.map((tool) => (
              <ToolCard
                key={tool.id}
                tool={tool}
                job={installJobs[tool.id]}
                onUpdated={(ts) => tools.setData((l) => upsertById(l, ts, false))}
                onInstallStarted={(job) => setInstallJobs((m) => ({ ...m, [tool.id]: job }))}
              />
            ))}
          </div>
        )}
      </AsyncView>
    </>
  );
}

function ToolCard({
  tool,
  job,
  onUpdated,
  onInstallStarted,
}: {
  tool: ToolStatus;
  job?: Job;
  onUpdated: (t: ToolStatus) => void;
  onInstallStarted: (j: Job) => void;
}) {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const [refreshing, setRefreshing] = useState(false);
  const [diag, setDiag] = useState<string[] | null>(null);
  const [diagBusy, setDiagBusy] = useState(false);

  return (
    <Card
      title={tool.name}
      actions={
        tool.installed ? <StatusPill status="installed" label={t('tools.installed')} /> : <StatusPill status="missing" label={t('tools.notInstalled')} />
      }
      data-testid={`tool-${tool.id}`}
    >
      <div className="flex flex-col gap-3 text-sm">
        <KeyValue
          items={[
            [t('tools.path'), tool.path ? <Code>{tool.path}</Code> : '—'],
            [t('tools.version'), tool.version ?? '—'],
            [
              t('tools.auth'),
              <span>
                <Badge tone={authTone(tool.authStatus)}>{t(`auth.${tool.authStatus}` as TKey)}</Badge>
                {tool.authDetail && <span className="ml-2 text-muted">{tool.authDetail}</span>}
              </span>,
            ],
            [t('tools.skillsDir'), tool.skillsDir ? <Code>{tool.skillsDir}</Code> : '—'],
            [t('tools.checked'), formatDate(tool.lastCheckedAt, lang)],
          ]}
        />
        {!tool.platformSupported && <p className="text-muted">{t('tools.unsupported')}</p>}
        {job && (
          <div className="rounded-md border border-line p-2">
            <div className="mb-1 flex items-center justify-between gap-2">
              <span>{t('tools.installJob')}</span>
              <StatusPill status={job.status} label={t(`job.status.${job.status}` as TKey)} />
            </div>
            <Progress value={job.status === 'queued' ? null : job.progress} ariaLabel={t('tools.installJob')} />
            {job.error && <p className="mt-1 text-danger">{job.error}</p>}
            <Link to="/jobs" className="text-xs text-primary underline">
              {t('tools.viewLog')}
            </Link>
          </div>
        )}
        {!tool.installed && tool.platformSupported && (
          <div>
            <p className="mb-2 font-semibold">{t('tools.methods')}</p>
            <HostOnly>
              <div className="flex flex-wrap items-center gap-2">
                {tool.installMethods.length === 0 ? (
                  <span className="text-muted">{t('tools.noMethods')}</span>
                ) : (
                  tool.installMethods.map((m) => <InstallMethodButton key={m.id} tool={tool} method={m} onStarted={onInstallStarted} />)
                )}
              </div>
            </HostOnly>
          </div>
        )}
        <div className="flex flex-wrap items-center gap-2">
          <HostOnly>
            <Button
              size="sm"
              loading={refreshing}
              onClick={async () => {
                setRefreshing(true);
                try {
                  const r = await api.tools.refresh(tool.id as ProviderId);
                  if (r && typeof r === 'object' && 'id' in r) onUpdated(r);
                } catch (e) {
                  notify.fail(e);
                } finally {
                  setRefreshing(false);
                }
              }}
            >
              {t('tools.refresh')}
            </Button>
            <Button
              size="sm"
              loading={diagBusy}
              onClick={async () => {
                setDiagBusy(true);
                try {
                  setDiag((await api.tools.diagnostics(tool.id)).lines);
                } catch (e) {
                  notify.fail(e);
                } finally {
                  setDiagBusy(false);
                }
              }}
            >
              {t('tools.diagnostics')}
            </Button>
          </HostOnly>
          <ExternalLink href={tool.docsUrl}>{t('tools.docs')}</ExternalLink>
        </div>
        {diag && <LogBox lines={diag} label={t('tools.diagnostics')} />}
      </div>
    </Card>
  );
}
