import { useMemo, useState } from 'react';
import { Badge, Button, ConfirmDialog, Drawer, EmptyState, Select, Tabs, TextInput, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { ProviderId, Skill, SkillSource } from '../api/types';
import { AsyncView, Code, ErrorBlock, HostOnly, KeyValue, LoadingBlock, PageHeader, useNotify } from '../components/common';
import { GithubTab, SourcesTab } from '../components/skill-sources';
import { useI18n, useT } from '../i18n';
import { formatBytes, formatDate } from '../lib/format';
import { useApi, useDebouncedValue } from '../lib/useApi';

const SKILL_PROVIDERS: ProviderId[] = ['claude', 'codex', 'antigravity'];
const PROVIDER_NAMES: Record<ProviderId, string> = { claude: 'Claude Code', codex: 'Codex', antigravity: 'Antigravity', cloudflared: 'cloudflared' };

export function SkillsPage() {
  const t = useT();
  const [tab, setTab] = useState('skills');
  const sources = useApi('sources', () => api.skills.sources());
  const [skillsVersion, setSkillsVersion] = useState(0);
  useServerEvent('skills', () => {
    void sources.reload();
    setSkillsVersion((v) => v + 1);
  });
  return (
    <>
      <PageHeader title={t('nav.skills')} description={t('skills.desc')} />
      <Tabs
        label={t('nav.skills')}
        value={tab}
        onChange={setTab}
        tabs={[
          { id: 'skills', label: t('skills.tab') },
          { id: 'sources', label: t('sources.tab') },
          { id: 'github', label: t('github.tab') },
        ]}
      >
        {tab === 'skills' && <SkillsTab sources={sources.data ?? []} version={skillsVersion} onGoSources={() => setTab('sources')} />}
        {tab === 'sources' && (
          <SourcesTab
            state={sources}
            onChanged={() => {
              void sources.reload();
              setSkillsVersion((v) => v + 1);
            }}
          />
        )}
        {tab === 'github' && <GithubTab />}
      </Tabs>
    </>
  );
}

type SkillFilter = 'all' | 'scripts' | 'warnings' | 'duplicates' | 'installed';

function SkillsTab({ sources, version, onGoSources }: { sources: SkillSource[]; version: number; onGoSources: () => void }) {
  const t = useT();
  const [q, setQ] = useState('');
  const debounced = useDebouncedValue(q.trim());
  const [sourceId, setSourceId] = useState('');
  const [filter, setFilter] = useState<SkillFilter>('all');
  const [openId, setOpenId] = useState<string | null>(null);
  const skills = useApi(`skills:${debounced}:${sourceId}:${version}`, () => api.skills.list(debounced || undefined, sourceId || undefined));
  const filtered = useMemo(() => {
    const l = skills.data ?? [];
    switch (filter) {
      case 'scripts':
        return l.filter((s) => s.hasScripts);
      case 'warnings':
        return l.filter((s) => s.warnings.length > 0);
      case 'duplicates':
        return l.filter((s) => s.duplicateOf.length > 0);
      case 'installed':
        return l.filter((s) => s.installedProviders.length > 0);
      default:
        return l;
    }
  }, [skills.data, filter]);
  const noFilters = !debounced && !sourceId && filter === 'all';

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-3 sm:grid-cols-3">
        <TextInput label={t('common.search')} type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('skills.searchPlaceholder')} />
        <Select
          label={t('skills.source')}
          value={sourceId}
          onChange={(e) => setSourceId(e.target.value)}
          options={[{ value: '', label: t('common.all') }, ...sources.map((s) => ({ value: s.id, label: s.displayName }))]}
        />
        <Select
          label={t('skills.filter')}
          value={filter}
          onChange={(e) => setFilter(e.target.value as SkillFilter)}
          options={[
            { value: 'all', label: t('common.all') },
            { value: 'installed', label: t('skills.filter.installed') },
            { value: 'scripts', label: t('skills.filter.scripts') },
            { value: 'warnings', label: t('skills.filter.warnings') },
            { value: 'duplicates', label: t('skills.filter.duplicates') },
          ]}
        />
      </div>
      <AsyncView
        state={skills}
        isEmpty={() => filtered.length === 0}
        empty={
          noFilters ? (
            <EmptyState
              icon="✦"
              title={t('skills.zeroTitle')}
              description={t('skills.zeroDesc')}
              action={<Button onClick={onGoSources}>{t('skills.goSources')}</Button>}
            />
          ) : (
            <EmptyState title={t('skills.noMatch')} />
          )
        }
      >
        {() => (
          <div className="overflow-x-auto">
            <p className="mb-2 text-sm text-muted" aria-live="polite">
              {t('skills.count', { n: filtered.length })}
            </p>
            <table className="data" data-testid="skills-table">
              <thead>
                <tr>
                  <th>{t('skills.name')}</th>
                  <th className="hidden md:table-cell">{t('skills.source')}</th>
                  <th>{t('skills.status')}</th>
                  <th className="hidden sm:table-cell">{t('skills.providers')}</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((s) => (
                  <tr key={s.id} className="clickable" onClick={() => setOpenId(s.id)}>
                    <td>
                      <button type="button" className="text-left font-semibold text-primary hover:underline" onClick={() => setOpenId(s.id)}>
                        {s.name}
                      </button>
                      <p className="line-clamp-2 text-xs text-muted">{s.description}</p>
                    </td>
                    <td className="hidden md:table-cell">{s.sourceName}</td>
                    <td>
                      <div className="flex flex-wrap gap-1">
                        {s.hasScripts && <Badge tone="warning">{t('skills.hasScripts')}</Badge>}
                        {s.warnings.length > 0 && <Badge tone="danger">{t('skills.warnings', { n: s.warnings.length })}</Badge>}
                        {s.duplicateOf.length > 0 && <Badge tone="info">{t('skills.duplicate')}</Badge>}
                      </div>
                    </td>
                    <td className="hidden sm:table-cell">
                      {s.installedProviders.length ? s.installedProviders.map((p) => PROVIDER_NAMES[p]).join(', ') : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </AsyncView>
      <SkillDrawer
        id={openId}
        onClose={() => setOpenId(null)}
        onChanged={(updated) => skills.setData((l) => l?.map((x) => (x.id === updated.id ? { ...x, installedProviders: updated.installedProviders } : x)))}
      />
    </div>
  );
}

function SkillDrawer({ id, onClose, onChanged }: { id: string | null; onClose: () => void; onChanged: (s: Skill) => void }) {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const detail = useApi(id ? `skill:${id}` : null, () => api.skills.get(id!));
  const [action, setAction] = useState<{ provider: ProviderId; kind: 'install' | 'uninstall' } | null>(null);
  const d = id && detail.data?.id === id ? detail.data : undefined;

  return (
    <Drawer open={id !== null} onClose={onClose} title={d?.name ?? t('skills.detail')} closeLabel={t('common.close')}>
      {!d ? (
        detail.error ? (
          <ErrorBlock message={detail.error} onRetry={() => void detail.reload()} />
        ) : (
          <LoadingBlock />
        )
      ) : (
        <div className="flex flex-col gap-5">
          <p className="text-sm">{d.description}</p>
          <KeyValue
            items={[
              [t('skills.source'), d.sourceName],
              [t('skills.path'), <Code>{d.relPath}</Code>],
              [t('skills.canonical'), <Code>{d.canonicalId}</Code>],
              [t('skills.revision'), <Code>{d.revision}</Code>],
              [t('skills.updated'), formatDate(d.updatedAt, lang)],
            ]}
          />
          {d.hasScripts && (
            <p className="rounded-md bg-warning-bg p-3 text-sm text-warning" role="note">
              ⚠ {t('skills.scriptsWarning')}
            </p>
          )}
          {d.warnings.length > 0 && (
            <section>
              <h3 className="mb-1 text-sm font-semibold">{t('skills.warningsTitle')}</h3>
              <ul className="list-disc pl-5 text-sm text-danger">
                {d.warnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            </section>
          )}
          {d.duplicateOf.length > 0 && (
            <section>
              <h3 className="mb-1 text-sm font-semibold">{t('skills.duplicatesTitle')}</h3>
              <p className="text-sm text-muted">{t('skills.duplicatesDesc', { ids: d.duplicateOf.join(', ') })}</p>
            </section>
          )}
          <section>
            <h3 className="mb-2 text-sm font-semibold">{t('skills.installTitle')}</h3>
            <div className="flex flex-col gap-2">
              {SKILL_PROVIDERS.map((p) => {
                const installed = d.installedProviders.includes(p);
                return (
                  <div key={p} className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-line px-3 py-2">
                    <span className="flex items-center gap-2 text-sm font-medium">
                      {PROVIDER_NAMES[p]}
                      {installed ? <StatusPill status="installed" label={t('skills.installed')} /> : <Badge>{t('skills.notInstalled')}</Badge>}
                    </span>
                    <HostOnly compact>
                      {installed ? (
                        <Button size="sm" variant="secondary" onClick={() => setAction({ provider: p, kind: 'uninstall' })}>
                          {t('skills.uninstall')}
                        </Button>
                      ) : (
                        <Button size="sm" variant="primary" onClick={() => setAction({ provider: p, kind: 'install' })}>
                          {t('skills.installFor', { provider: PROVIDER_NAMES[p] })}
                        </Button>
                      )}
                    </HostOnly>
                  </div>
                );
              })}
            </div>
          </section>
          <section>
            <h3 className="mb-1 text-sm font-semibold">SKILL.md</h3>
            <pre className="max-h-96 overflow-auto rounded-md bg-surface-2 p-3 font-mono text-xs whitespace-pre-wrap break-words" tabIndex={0}>
              {d.content}
            </pre>
          </section>
          <section>
            <h3 className="mb-1 text-sm font-semibold">{t('skills.files', { n: d.files.length })}</h3>
            <ul className="text-sm">
              {d.files.map((f) => (
                <li key={f.path} className="flex justify-between gap-2 border-b border-line py-1">
                  <span className="font-mono text-xs break-all">
                    {f.path} {f.executable && <Badge tone="warning">{t('skills.executable')}</Badge>}
                  </span>
                  <span className="shrink-0 text-xs text-muted">{formatBytes(f.size)}</span>
                </li>
              ))}
            </ul>
          </section>
        </div>
      )}
      {d && action && (
        <ConfirmDialog
          open
          tone={action.kind === 'uninstall' ? 'danger' : 'primary'}
          title={
            action.kind === 'install'
              ? t('skills.installConfirmTitle', { name: d.name, provider: PROVIDER_NAMES[action.provider] })
              : t('skills.uninstallConfirmTitle', { name: d.name, provider: PROVIDER_NAMES[action.provider] })
          }
          confirmLabel={action.kind === 'install' ? t('skills.install') : t('skills.uninstall')}
          cancelLabel={t('common.cancel')}
          acknowledge={action.kind === 'install' && d.hasScripts ? t('skills.ackScripts') : undefined}
          onCancel={() => setAction(null)}
          onConfirm={async () => {
            if (action.kind === 'install') await api.skills.install(d.id, action.provider, d.hasScripts);
            else await api.skills.uninstall(d.id, action.provider);
            const fresh = await api.skills.get(d.id);
            detail.setData(fresh);
            onChanged(fresh);
            setAction(null);
            notify.ok(action.kind === 'install' ? t('skills.installedOk') : t('skills.uninstalledOk'));
          }}
        >
          <p>{action.kind === 'install' ? t('skills.installConfirmBody') : t('skills.uninstallConfirmBody')}</p>
          {action.kind === 'install' && d.hasScripts && <p className="mt-2 text-warning">⚠ {t('skills.scriptsWarning')}</p>}
        </ConfirmDialog>
      )}
    </Drawer>
  );
}
