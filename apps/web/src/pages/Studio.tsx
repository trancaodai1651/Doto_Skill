import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router';
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, Select, StatusPill, TextArea, TextInput } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type {
  AiProviderId,
  StudioProviderCatalog,
  StudioRunDetail,
  StudioRunMode,
  StudioRunRequest,
  StudioSession,
  StudioSessionSpec,
} from '../api/types';
import { AsyncView, Code, HostOnly, LogBox, PageHeader, useNotify } from '../components/common';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { formatBytes, formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';

const AI_ORDER: AiProviderId[] = ['antigravity', 'claude', 'codex'];

export function StudioPage() {
  const { t } = useI18n();
  const catalog = useApi('studio-catalog', () => api.studio.catalog());
  const skills = useApi('studio-skills', () => api.skills.list());
  const uploads = useApi('studio-uploads', () => api.uploads.list());
  const runs = useApi('studio-runs', () => api.studio.runs());

  useServerEvent('studio', (sess: StudioSession) => {
    runs.setData((prev) =>
      prev?.map((r) =>
        r.job.id === sess.jobId ? { ...r, sessions: r.sessions.map((s) => (s.id === sess.id ? sess : s)) } : r,
      ),
    );
  });
  useServerEvent('job', (j) => {
    if (j.kind !== 'studio') return;
    runs.setData((prev) => prev?.map((r) => (r.job.id === j.id ? { ...r, job: j } : r)));
  });

  const hasRunning = (runs.data ?? []).some((r) => r.job.status === 'running' || r.job.status === 'queued');
  useEffect(() => {
    if (!hasRunning) return;
    const h = setInterval(() => void runs.reload(), 2500);
    return () => clearInterval(h);
  }, [hasRunning, runs]);

  return (
    <>
      <PageHeader
        title={t('nav.studio')}
        description={t('studio.desc')}
        actions={
          <Button size="sm" onClick={() => void Promise.all([catalog.reload(), runs.reload(), uploads.reload()])}>
            {t('common.reload')}
          </Button>
        }
      />
      <div className="flex flex-col gap-5">
        <AsyncView state={catalog}>
          {(cat) => (
            <StudioComposer
              providers={cat.providers}
              skills={skills.data ?? []}
              uploads={uploads.data ?? []}
              onUploadsChanged={() => void uploads.reload()}
              onStarted={(run) => runs.setData((prev) => [run, ...(prev ?? [])])}
            />
          )}
        </AsyncView>

        <Card title={t('studio.runsTitle')} description={t('studio.runsDesc')}>
          <AsyncView
            state={runs}
            isEmpty={(l) => l.length === 0}
            empty={<EmptyState icon="⚡" title={t('studio.emptyRuns')} description={t('studio.emptyRunsDesc')} />}
          >
            {(list) => (
              <div className="flex flex-col gap-5" data-testid="studio-runs-list">
                {list.map((r) => (
                  <RunMonitorCard key={r.job.id} run={r} onChanged={() => void runs.reload()} />
                ))}
              </div>
            )}
          </AsyncView>
        </Card>
      </div>
    </>
  );
}

function StudioComposer({
  providers,
  skills,
  uploads,
  onUploadsChanged,
  onStarted,
}: {
  providers: StudioProviderCatalog[];
  skills: { id: string; name: string; canonicalId: string }[];
  uploads: { id: string; originalName: string; mediaKind: 'video' | 'image'; sizeBytes: number }[];
  onUploadsChanged: () => void;
  onStarted: (r: StudioRunDetail) => void;
}) {
  const { t } = useI18n();
  const notify = useNotify();
  const installedFirst = providers.find((p) => p.installed)?.id ?? 'claude';
  const [mode, setMode] = useState<StudioRunMode>('single');
  const [provider, setProvider] = useState<AiProviderId>(installedFirst);
  const [model, setModel] = useState('');
  const [effort, setEffort] = useState('high');
  const [count, setCount] = useState('2');
  const [multiEnabled, setMultiEnabled] = useState<Record<AiProviderId, boolean>>({
    antigravity: Boolean(providers.find((p) => p.id === 'antigravity')?.installed),
    claude: Boolean(providers.find((p) => p.id === 'claude')?.installed),
    codex: Boolean(providers.find((p) => p.id === 'codex')?.installed),
  });
  const [multiModel, setMultiModel] = useState<Record<AiProviderId, string>>({ antigravity: '', claude: '', codex: '' });
  const [multiEffort, setMultiEffort] = useState<Record<AiProviderId, string>>({ antigravity: 'high', claude: 'high', codex: 'high' });

  const [skillId, setSkillId] = useState('');
  const [title, setTitle] = useState('');
  const [prompt, setPrompt] = useState('');
  const [selectedUploads, setSelectedUploads] = useState<string[]>([]);
  const [includeMemoryHint, setIncludeMemoryHint] = useState(true);
  const [visibility, setVisibility] = useState<'private' | 'viewers'>('private');
  const [uploading, setUploading] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const curProvider = useMemo(() => providers.find((p) => p.id === provider) ?? providers[0], [providers, provider]);
  const curModels = useMemo(() => curProvider?.models ?? [], [curProvider]);
  const curEfforts = useMemo(() => {
    const found = curModels.find((m) => m.id === model) ?? curModels[0];
    return found?.efforts?.length ? found.efforts : ['low', 'medium', 'high', 'xhigh'];
  }, [curModels, model]);

  function buildSessionsPreview(): StudioSessionSpec[] {
    if (mode === 'single') {
      return [{ provider, model, effort, label: 'Phiên #1' }];
    }
    if (mode === 'multi_same_ai') {
      const n = Math.max(2, Math.min(6, Number(count) || 2));
      return Array.from({ length: n }, (_, i) => ({ provider, model, effort, label: `Phương án #${i + 1}` }));
    }
    return AI_ORDER.filter((id) => multiEnabled[id]).map((id) => ({
      provider: id,
      model: multiModel[id],
      effort: multiEffort[id],
      label: id.toUpperCase(),
    }));
  }

  const previewSessions = buildSessionsPreview();

  function validateBeforeConfirm() {
    if (!prompt.trim()) {
      setErr(t('studio.promptRequired'));
      return;
    }
    if (previewSessions.length === 0) {
      setErr(t('studio.selectAtLeastOneAi'));
      return;
    }
    const missing = previewSessions.find((s) => !providers.find((p) => p.id === s.provider)?.installed);
    if (missing) {
      setErr(t('studio.cliMissing', { provider: missing.provider }));
      return;
    }
    setErr(null);
    setConfirmOpen(true);
  }

  return (
    <Card title={t('studio.composeTitle')} description={t('studio.composeDesc')}>
      <div className="mb-4 flex flex-wrap items-center gap-2">
        {providers.map((p) => (
          <span key={p.id} className="inline-flex items-center gap-1.5 rounded-md border border-line bg-surface-2 px-2.5 py-1 text-xs">
            <strong>{p.name}</strong>
            <StatusPill status={p.installed ? 'installed' : 'missing'} label={p.installed ? p.version ?? t('tools.installed') : t('tools.notInstalled')} />
          </span>
        ))}
        <Link to="/tools" className="text-xs text-primary underline">
          {t('studio.manageTools')}
        </Link>
      </div>

      <div className="flex flex-col gap-4">
        <div className="grid gap-3 sm:grid-cols-3">
          <Select
            label={t('studio.modeLabel')}
            value={mode}
            onChange={(e) => setMode(e.target.value as StudioRunMode)}
            options={[
              { value: 'single', label: t('studio.mode.single') },
              { value: 'multi_same_ai', label: t('studio.mode.multi_same_ai') },
              { value: 'multi_ai', label: t('studio.mode.multi_ai') },
            ]}
          />
          <Select
            label={t('skills.name')}
            value={skillId}
            onChange={(e) => setSkillId(e.target.value)}
            options={[{ value: '', label: t('studio.noSkill') }, ...skills.map((s) => ({ value: s.id, label: `${s.name} (${s.canonicalId})` }))]}
          />
          <TextInput label={t('studio.runTitle')} placeholder={t('studio.runTitlePlaceholder')} value={title} onChange={(e) => setTitle(e.target.value)} />
        </div>

        {(mode === 'single' || mode === 'multi_same_ai') && (
          <div className="grid gap-3 rounded-md border border-line bg-surface-2/40 p-3 sm:grid-cols-4">
            <Select
              label="AI CLI"
              value={provider}
              onChange={(e) => {
                setProvider(e.target.value as AiProviderId);
                setModel('');
              }}
              options={providers.map((p) => ({
                value: p.id,
                label: `${p.name}${p.installed ? '' : ` (${t('tools.notInstalled')})`}`,
              }))}
            />
            <Select
              label="Model"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              options={[{ value: '', label: t('render.default') }, ...curModels.map((m) => ({ value: m.id, label: m.name }))]}
            />
            <Select
              label="Effort"
              value={effort}
              onChange={(e) => setEffort(e.target.value)}
              options={curEfforts.map((ef) => ({ value: ef, label: ef.toUpperCase() }))}
            />
            {mode === 'multi_same_ai' && (
              <Select
                label={t('studio.sessionCount')}
                value={count}
                onChange={(e) => setCount(e.target.value)}
                options={['2', '3', '4', '5', '6'].map((n) => ({ value: n, label: `${n} ${t('studio.sessionsUnit')}` }))}
              />
            )}
          </div>
        )}

        {mode === 'multi_ai' && (
          <div className="grid gap-3 md:grid-cols-3">
            {providers.map((p) => (
              <div key={p.id} className="rounded-md border border-line bg-surface-2/40 p-3">
                <Checkbox
                  label={`${p.name} (${p.vendor})`}
                  checked={multiEnabled[p.id]}
                  onChange={(e) => setMultiEnabled((prev) => ({ ...prev, [p.id]: e.target.checked }))}
                />
                {multiEnabled[p.id] && (
                  <div className="mt-2 flex flex-col gap-2">
                    <Select
                      label="Model"
                      value={multiModel[p.id]}
                      onChange={(e) => setMultiModel((prev) => ({ ...prev, [p.id]: e.target.value }))}
                      options={[{ value: '', label: t('render.default') }, ...p.models.map((m) => ({ value: m.id, label: m.name }))]}
                    />
                    <Select
                      label="Effort"
                      value={multiEffort[p.id]}
                      onChange={(e) => setMultiEffort((prev) => ({ ...prev, [p.id]: e.target.value }))}
                      options={['low', 'medium', 'high', 'xhigh'].map((ef) => ({ value: ef, label: ef.toUpperCase() }))}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        <TextArea
          label={t('studio.promptLabel')}
          placeholder={t('studio.promptPlaceholder')}
          rows={4}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          error={err ?? undefined}
        />

        <div className="rounded-md border border-line p-3 text-sm">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="font-semibold">{t('studio.attachTitle')}</span>
            <label className="ui-btn ui-btn--secondary cursor-pointer text-xs">
              {uploading ? t('common.loading') : `📱 ${t('uploads.pickFiles')}`}
              <input
                type="file"
                accept="video/*,image/*"
                multiple
                className="sr-only"
                disabled={uploading}
                onChange={async (e) => {
                  const files = Array.from(e.target.files ?? []);
                  e.target.value = '';
                  if (!files.length) return;
                  setUploading(true);
                  try {
                    const addedIds: string[] = [];
                    for (const f of files) {
                      const r = await api.uploads.upload(f);
                      addedIds.push(r.upload.id);
                    }
                    onUploadsChanged();
                    setSelectedUploads((prev) => [...new Set([...prev, ...addedIds])]);
                    notify.ok(t('uploads.uploadedCount', { n: files.length }));
                  } catch (ex) {
                    notify.fail(ex);
                  } finally {
                    setUploading(false);
                  }
                }}
              />
            </label>
          </div>
          {uploads.length === 0 ? (
            <p className="mt-1 text-xs text-muted">{t('studio.attachEmpty')}</p>
          ) : (
            <div className="mt-2 flex max-h-32 flex-wrap gap-2 overflow-y-auto">
              {uploads.slice(0, 24).map((u) => {
                const checked = selectedUploads.includes(u.id);
                return (
                  <label key={u.id} className="inline-flex cursor-pointer items-center gap-1.5 rounded border border-line bg-surface-2 px-2 py-1 text-xs">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => setSelectedUploads((prev) => (checked ? prev.filter((x) => x !== u.id) : [...prev, u.id]))}
                    />
                    <span>{u.mediaKind === 'video' ? '🎬' : '🖼'} {u.originalName}</span>
                    <span className="text-muted">({formatBytes(u.sizeBytes)})</span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-4">
          <Checkbox label={t('studio.includeMemory')} checked={includeMemoryHint} onChange={(e) => setIncludeMemoryHint(e.target.checked)} />
          <Select
            label={t('render.visibility')}
            value={visibility}
            onChange={(e) => setVisibility(e.target.value as 'private' | 'viewers')}
            options={[
              { value: 'private', label: t('videos.private') },
              { value: 'viewers', label: t('videos.visibleViewers') },
            ]}
          />
        </div>

        <div>
          <HostOnly>
            <Button variant="primary" onClick={validateBeforeConfirm} data-testid="studio-launch-btn">
              ⚡ {t('studio.launchBtn', { n: previewSessions.length })}
            </Button>
          </HostOnly>
        </div>
      </div>

      <ConfirmDialog
        open={confirmOpen}
        wide
        title={t('studio.confirmTitle', { n: previewSessions.length })}
        confirmLabel={t('studio.confirmRun')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmOpen(false)}
        onConfirm={async () => {
          const req: StudioRunRequest = {
            mode,
            prompt: prompt.trim(),
            title: title.trim() || undefined,
            skillId: skillId || null,
            provider,
            model,
            effort,
            count: Number(count) || 2,
            sessions: mode === 'multi_ai' ? previewSessions : undefined,
            uploadIds: selectedUploads,
            includeMemoryHint,
            visibility,
            confirm: true,
          };
          const started = await api.studio.start(req);
          setConfirmOpen(false);
          setPrompt('');
          notify.ok(t('studio.startedOk'), started.job.title);
          onStarted(started);
        }}
      >
        <p className="text-sm">{t('studio.confirmBody')}</p>
        <ul className="mt-2 list-disc space-y-1 pl-5 text-xs">
          {previewSessions.map((s, idx) => (
            <li key={idx}>
              <strong>{s.label ?? `#${idx + 1}`}</strong>: <Code>{s.provider}</Code>{' '}
              {s.model && <Badge>{s.model}</Badge>} {s.effort && <Badge tone="info">{s.effort}</Badge>}
            </li>
          ))}
        </ul>
      </ConfirmDialog>
    </Card>
  );
}

function RunMonitorCard({ run, onChanged }: { run: StudioRunDetail; onChanged: () => void }) {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const [stopping, setStopping] = useState<string | null>(null);
  const active = run.job.status === 'running' || run.job.status === 'queued';

  return (
    <section className="rounded-lg border border-line bg-surface-2/30 p-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="font-bold">{run.job.title}</h3>
          <p className="text-xs text-muted">
            {formatDate(run.job.createdAt, lang)} · {run.sessions.length} {t('studio.sessionsUnit')}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <StatusPill status={run.job.status} label={t(`job.status.${run.job.status}` as TKey)} />
          {active && (
            <Button
              size="sm"
              variant="danger"
              loading={stopping === 'all'}
              onClick={async () => {
                setStopping('all');
                try {
                  await api.studio.cancelRun(run.job.id);
                  onChanged();
                } catch (e) {
                  notify.fail(e);
                } finally {
                  setStopping(null);
                }
              }}
            >
              {t('studio.stopAll')}
            </Button>
          )}
        </div>
      </div>

      <div className={`grid gap-3 ${run.sessions.length > 1 ? 'lg:grid-cols-2' : 'grid-cols-1'}`}>
        {run.sessions.map((s) => (
          <div key={s.id} className="flex flex-col gap-2 rounded-md border border-line bg-surface p-3 text-sm">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex flex-wrap items-center gap-1.5">
                <strong>{s.label}</strong>
                <Badge tone="info">{s.provider}</Badge>
                {s.model && <Badge>{s.model}</Badge>}
                {s.effort && <Badge>{s.effort}</Badge>}
              </div>
              <div className="flex items-center gap-2">
                <StatusPill status={s.status} label={t(`job.status.${s.status}` as TKey)} />
                {s.status === 'running' && (
                  <Button
                    size="sm"
                    variant="ghost"
                    loading={stopping === s.id}
                    onClick={async () => {
                      setStopping(s.id);
                      try {
                        await api.studio.cancelSession(s.id);
                        onChanged();
                      } catch (e) {
                        notify.fail(e);
                      } finally {
                        setStopping(null);
                      }
                    }}
                  >
                    {t('server.stop')}
                  </Button>
                )}
              </div>
            </div>
            <p className="text-xs text-muted">
              <Code>{s.commandPreview}</Code>
            </p>
            {s.error && <p className="text-xs text-danger">{s.error}</p>}
            {s.resultText && (
              <div className="rounded bg-info-bg/50 p-2.5 text-xs whitespace-pre-wrap break-words">{s.resultText}</div>
            )}
            {s.outputVideoIds.length > 0 && (
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <span className="font-semibold text-success">🎬 {t('studio.exportedVideos')}:</span>
                {s.outputVideoIds.map((vid, idx) => (
                  <Link key={vid} to={`/gallery/${vid}`} className="font-semibold text-primary underline">
                    Video #{idx + 1}
                  </Link>
                ))}
              </div>
            )}
            <details>
              <summary className="cursor-pointer text-xs font-medium text-muted">
                {t('jobs.log')} ({s.log.length})
              </summary>
              <div className="mt-1">
                <LogBox lines={s.log} label={`${s.label} log`} />
              </div>
            </details>
          </div>
        ))}
      </div>
    </section>
  );
}
