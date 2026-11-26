import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, Progress, TextInput, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import type { AppSettings, ServerMode } from '../api/types';
import { AsyncView, Code, ErrorBlock, ExternalLink, KeyValue, LoadingBlock, useNotify } from '../components/common';
import {
  AddFolderSourceForm,
  AddGithubSourceForm,
  HardwareView,
  InstallMethodButton,
  LanConfirmDialog,
  LimitsTable,
  ProfilePicker,
  authTone,
  isAbsolutePath,
  useResourceSettings,
} from '../components/features';
import { Toolbar } from '../components/Layout';
import { useT } from '../i18n';
import type { TKey } from '../i18n';
import { useApi } from '../lib/useApi';
import { useSession } from '../state/session';

const STEPS: TKey[] = [
  'onb.step.terms',
  'onb.step.hardware',
  'onb.step.profile',
  'onb.step.folders',
  'onb.step.skills',
  'onb.step.tools',
  'onb.step.server',
  'onb.step.review',
];
const SKIPPABLE = new Set([3, 4, 5]);

export function OnboardingPage() {
  const t = useT();
  const [step, setStep] = useState(0);
  const [accepted, setAccepted] = useState(false);
  const last = STEPS.length - 1;
  const canNext = step !== 0 || accepted;

  return (
    <div className="min-h-dvh">
      <header className="flex items-center justify-between gap-2 border-b border-line bg-surface px-4 py-2">
        <span className="flex items-center gap-2 font-bold">
          <img src="/icons/icon.svg" alt="" width={24} height={24} /> Doto Skill · {t('onb.title')}
        </span>
        <Toolbar />
      </header>
      <main className="mx-auto w-full max-w-3xl px-4 py-6">
        <nav aria-label={t('onb.progress')} className="mb-4">
          <p className="mb-2 text-sm text-muted" aria-live="polite">
            {t('onb.stepOf', { n: step + 1, total: STEPS.length })} · <strong className="text-fg">{t(STEPS[step]!)}</strong>
          </p>
          <Progress value={(step + 1) / STEPS.length} ariaLabel={t('onb.progress')} label=" " />
          <ol className="mt-3 hidden flex-wrap gap-2 text-xs sm:flex">
            {STEPS.map((s, i) => (
              <li key={s} aria-current={i === step ? 'step' : undefined}>
                <Badge tone={i === step ? 'info' : i < step ? 'success' : 'neutral'}>
                  {i + 1}. {t(s)}
                </Badge>
              </li>
            ))}
          </ol>
        </nav>

        <section aria-labelledby="onb-step-title">
          <h1 id="onb-step-title" className="mb-3 text-xl font-bold">
            {t(STEPS[step]!)}
          </h1>
          {step === 0 && <StepTerms accepted={accepted} setAccepted={setAccepted} />}
          {step === 1 && <StepHardware />}
          {step === 2 && <StepProfile />}
          {step === 3 && <StepFolders />}
          {step === 4 && <StepSkills />}
          {step === 5 && <StepTools />}
          {step === 6 && <StepServer />}
          {step === 7 && <StepReview />}
        </section>

        {step < last && (
          <div className="mt-6 flex flex-wrap items-center justify-between gap-2">
            <Button variant="ghost" onClick={() => setStep((s) => Math.max(0, s - 1))} disabled={step === 0}>
              {t('onb.back')}
            </Button>
            <div className="flex gap-2">
              {SKIPPABLE.has(step) && (
                <Button variant="secondary" onClick={() => setStep((s) => s + 1)}>
                  {t('onb.skip')}
                </Button>
              )}
              <Button variant="primary" onClick={() => setStep((s) => s + 1)} disabled={!canNext}>
                {t('onb.next')}
              </Button>
            </div>
          </div>
        )}
        {step === last && (
          <div className="mt-6">
            <Button variant="ghost" onClick={() => setStep((s) => s - 1)}>
              {t('onb.back')}
            </Button>
          </div>
        )}
      </main>
    </div>
  );
}

function StepTerms({ accepted, setAccepted }: { accepted: boolean; setAccepted: (v: boolean) => void }) {
  const t = useT();
  const settings = useApi('settings', () => api.settings.get());
  return (
    <Card>
      <div className="space-y-3 text-sm leading-relaxed">
        <p>{t('onb.terms.p1')}</p>
        <ul className="list-disc space-y-1 pl-5">
          <li>{t('onb.terms.l1')}</li>
          <li>{t('onb.terms.l2')}</li>
          <li>{t('onb.terms.l3')}</li>
          <li>{t('onb.terms.l4')}</li>
        </ul>
        <div>
          <p className="font-semibold">{t('onb.terms.dataDir')}</p>
          <AsyncView state={settings}>{(s) => <Code>{s.dataDir}</Code>}</AsyncView>
        </div>
        <Checkbox label={t('onb.terms.accept')} checked={accepted} onChange={(e) => setAccepted(e.target.checked)} />
      </div>
    </Card>
  );
}

function StepHardware() {
  const t = useT();
  const notify = useNotify();
  const hw = useApi('hw', () => api.system.hardware());
  const [busy, setBusy] = useState(false);
  return (
    <Card
      title={t('hw.title')}
      actions={
        <Button
          size="sm"
          loading={busy}
          onClick={async () => {
            setBusy(true);
            try {
              const r = await api.system.probeHardware();
              if (r && typeof r === 'object' && 'cpuBrand' in r) hw.setData(r);
              else await hw.reload();
            } catch (e) {
              notify.fail(e);
            } finally {
              setBusy(false);
            }
          }}
        >
          {t('hw.probe')}
        </Button>
      }
    >
      <AsyncView state={hw}>{(d) => <HardwareView hw={d} />}</AsyncView>
    </Card>
  );
}

function StepProfile() {
  const t = useT();
  const notify = useNotify();
  const res = useResourceSettings();
  const [busy, setBusy] = useState(false);
  return (
    <Card description={t('onb.profile.desc')}>
      <AsyncView state={res}>
        {(r) => (
          <div className="flex flex-col gap-4">
            <ProfilePicker
              value={r.profile}
              busy={busy}
              onSelect={async (p) => {
                setBusy(true);
                try {
                  res.setData(await api.settings.putResources(p, r.overrides));
                } catch (e) {
                  notify.fail(e);
                } finally {
                  setBusy(false);
                }
              }}
            />
            <div className="overflow-x-auto">
              <h3 className="mb-2 text-sm font-semibold">{t('onb.profile.effective', { profile: t(`profile.${r.profile}` as TKey) })}</h3>
              <LimitsTable limits={r.effective} />
            </div>
          </div>
        )}
      </AsyncView>
    </Card>
  );
}

function StepFolders() {
  const settings = useApi('settings', () => api.settings.get());
  return <AsyncView state={settings}>{(s) => <FoldersForm settings={s} onSaved={(n) => settings.setData(n)} />}</AsyncView>;
}

export function FoldersForm({ settings, onSaved }: { settings: AppSettings; onSaved: (s: AppSettings) => void }) {
  const t = useT();
  const notify = useNotify();
  const [dirs, setDirs] = useState<string[]>(settings.videoSourceDirs);
  const [newDir, setNewDir] = useState('');
  const [output, setOutput] = useState(settings.outputDir ?? '');
  const [errs, setErrs] = useState<Record<string, string>>({});
  const [confirm, setConfirm] = useState(false);
  const dirty = JSON.stringify(dirs) !== JSON.stringify(settings.videoSourceDirs) || (output.trim() || null) !== settings.outputDir;

  function addDir() {
    const d = newDir.trim();
    if (!d) return;
    if (!isAbsolutePath(d)) {
      setErrs({ newDir: t('path.absolute') });
      return;
    }
    setErrs({});
    if (!dirs.includes(d)) setDirs([...dirs, d]);
    setNewDir('');
  }

  function save() {
    if (output.trim() && !isAbsolutePath(output.trim())) {
      setErrs({ output: t('path.absolute') });
      return;
    }
    setErrs({});
    setConfirm(true);
  }

  return (
    <Card description={t('folders.desc')}>
      <div className="flex flex-col gap-4">
        <div>
          <p className="mb-2 text-sm font-semibold">{t('folders.sources')}</p>
          {dirs.length === 0 ? (
            <p className="text-sm text-muted">{t('folders.none')}</p>
          ) : (
            <ul className="flex flex-col gap-1">
              {dirs.map((d) => (
                <li key={d} className="flex items-center justify-between gap-2 rounded-md bg-surface-2 px-3 py-1.5">
                  <Code>{d}</Code>
                  <Button size="sm" variant="ghost" onClick={() => setDirs(dirs.filter((x) => x !== d))} aria-label={t('folders.remove', { dir: d })}>
                    ✕
                  </Button>
                </li>
              ))}
            </ul>
          )}
          <div className="mt-2 flex flex-col gap-2 sm:flex-row sm:items-end">
            <TextInput
              fieldClassName="flex-1"
              label={t('folders.addSource')}
              placeholder={t('sources.folderPlaceholder')}
              value={newDir}
              onChange={(e) => setNewDir(e.target.value)}
              error={errs.newDir}
              spellCheck={false}
              autoCapitalize="none"
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  addDir();
                }
              }}
            />
            <Button onClick={addDir}>{t('folders.add')}</Button>
          </div>
        </div>
        <TextInput
          label={t('folders.output')}
          placeholder={t('sources.folderPlaceholder')}
          value={output}
          onChange={(e) => setOutput(e.target.value)}
          error={errs.output}
          hint={t('folders.outputHint')}
          spellCheck={false}
          autoCapitalize="none"
        />
        <div>
          <Button variant="primary" disabled={!dirty} onClick={save}>
            {t('common.save')}
          </Button>
        </div>
      </div>
      <ConfirmDialog
        open={confirm}
        title={t('folders.confirmTitle')}
        confirmLabel={t('common.save')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirm(false)}
        onConfirm={async () => {
          const next: AppSettings = { ...settings, videoSourceDirs: dirs, outputDir: output.trim() || null };
          const r = await api.settings.put(next);
          setConfirm(false);
          onSaved(r && typeof r === 'object' && 'dataDir' in r ? r : next);
          notify.ok(t('common.saved'));
        }}
      >
        <p>{t('folders.confirmBody')}</p>
        <ul className="mt-2 list-disc pl-5 text-sm">
          {dirs.map((d) => (
            <li key={d}>
              <Code>{d}</Code>
            </li>
          ))}
          {output.trim() && (
            <li>
              {t('folders.output')}: <Code>{output.trim()}</Code>
            </li>
          )}
        </ul>
      </ConfirmDialog>
    </Card>
  );
}

function StepSkills() {
  const t = useT();
  const sources = useApi('sources', () => api.skills.sources());
  return (
    <div className="flex flex-col gap-4">
      {(sources.data?.length ?? 0) === 0 && <EmptyState icon="✦" title={t('skills.zeroTitle')} description={t('skills.zeroDesc')} />}
      <AsyncView state={sources}>
        {(list) =>
          list.length > 0 ? (
            <Card title={t('sources.title')}>
              <ul className="flex flex-col gap-1 text-sm" data-testid="onb-sources">
                {list.map((s) => (
                  <li key={s.id} className="flex flex-wrap items-center gap-2">
                    <Badge tone="info">{s.kind}</Badge> {s.displayName} <span className="text-muted">({t('sources.skillCount', { n: s.skillCount })})</span>
                  </li>
                ))}
              </ul>
            </Card>
          ) : null
        }
      </AsyncView>
      <div className="grid gap-4 md:grid-cols-2">
        <Card title={t('sources.addFolder')}>
          <AddFolderSourceForm onAdded={() => void sources.reload()} />
        </Card>
        <Card title={t('sources.addGithub')}>
          <AddGithubSourceForm onAdded={() => void sources.reload()} />
        </Card>
      </div>
    </div>
  );
}

function StepTools() {
  const t = useT();
  const tools = useApi('tools', () => api.tools.list());
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted">{t('onb.tools.desc')}</p>
      <AsyncView state={tools} isEmpty={(l) => l.length === 0} empty={<EmptyState title={t('tools.empty')} />}>
        {(list) =>
          list.map((tool) => (
            <Card
              key={tool.id}
              title={tool.name}
              actions={
                tool.installed ? (
                  <StatusPill status="installed" label={t('tools.installed')} />
                ) : (
                  <StatusPill status="missing" label={t('tools.notInstalled')} />
                )
              }
            >
              <div className="flex flex-col gap-2 text-sm">
                {tool.installed && (
                  <KeyValue
                    items={[
                      [t('tools.version'), tool.version ?? '—'],
                      [t('tools.auth'), <Badge tone={authTone(tool.authStatus)}>{t(`auth.${tool.authStatus}` as TKey)}</Badge>],
                    ]}
                  />
                )}
                {!tool.platformSupported && <p className="text-muted">{t('tools.unsupported')}</p>}
                {!tool.installed && tool.platformSupported && (
                  <div className="flex flex-wrap items-center gap-2">
                    {tool.installMethods.length === 0 ? (
                      <span className="text-muted">{t('tools.noMethods')}</span>
                    ) : (
                      tool.installMethods.map((m) => <InstallMethodButton key={m.id} tool={tool} method={m} onStarted={() => void tools.reload()} />)
                    )}
                  </div>
                )}
                <p>
                  <ExternalLink href={tool.docsUrl}>{t('tools.docs')}</ExternalLink>
                </p>
              </div>
            </Card>
          ))
        }
      </AsyncView>
    </div>
  );
}

function StepServer() {
  const t = useT();
  const notify = useNotify();
  const cfg = useApi('server-config', () => api.server.config());
  const [mode, setMode] = useState<ServerMode | null>(null);
  const [lanConfirm, setLanConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [applied, setApplied] = useState<ServerMode | null>(null);

  if (cfg.data === undefined) return cfg.error ? <ErrorBlock message={cfg.error} onRetry={() => void cfg.reload()} /> : <LoadingBlock />;
  const current = cfg.data;
  const selected = mode ?? current.mode;

  async function apply(m: ServerMode) {
    setBusy(true);
    try {
      const next = { ...current, mode: m };
      const r = await api.server.putConfig(next);
      cfg.setData(r && typeof r === 'object' && 'mode' in r ? r : next);
      if (m === 'off') await api.server.stop().catch(() => undefined);
      else await api.server.restart();
      setApplied(m);
      notify.ok(t('server.applied'));
    } catch (e) {
      notify.fail(e);
      throw e;
    } finally {
      setBusy(false);
    }
  }

  const modes: ServerMode[] = ['off', 'localhost', 'lan'];
  return (
    <Card description={t('onb.server.desc')}>
      <div role="radiogroup" aria-label={t('server.mode')} className="flex flex-col gap-2">
        {modes.map((m) => (
          <label key={m} className="flex cursor-pointer items-start gap-3 rounded-md border border-line p-3 has-[:checked]:border-primary has-[:checked]:bg-info-bg">
            <input type="radio" name="server-mode" className="mt-1" checked={selected === m} onChange={() => setMode(m)} />
            <span>
              <span className="font-semibold">{t(`server.mode.${m}` as TKey)}</span>
              <span className="block text-sm text-muted">{t(`server.mode.${m}.desc` as TKey)}</span>
            </span>
          </label>
        ))}
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button
          variant="primary"
          loading={busy}
          onClick={() => {
            if (selected === 'lan') setLanConfirm(true);
            else void apply(selected).catch(() => undefined);
          }}
        >
          {t('server.apply')}
        </Button>
        {applied && <Badge tone="success">{t('server.appliedMode', { mode: t(`server.mode.${applied}` as TKey) })}</Badge>}
      </div>
      <p className="mt-4 rounded-md bg-surface-2 p-3 text-sm">{t('onb.server.cloudflare')}</p>
      <LanConfirmDialog
        open={lanConfirm}
        onCancel={() => setLanConfirm(false)}
        onConfirm={async () => {
          await apply('lan');
          setLanConfirm(false);
        }}
      />
    </Card>
  );
}

function StepReview() {
  const t = useT();
  const navigate = useNavigate();
  const { refresh } = useSession();
  const notify = useNotify();
  const summary = useApi('review', async () => {
    const [settings, resources, sources, tools, server] = await Promise.all([
      api.settings.get(),
      api.settings.resources(),
      api.skills.sources(),
      api.tools.list(),
      api.server.config(),
    ]);
    return { settings, resources, sources, tools, server };
  });
  const [busy, setBusy] = useState(false);
  return (
    <Card>
      <AsyncView state={summary}>
        {(s) => (
          <KeyValue
            items={[
              [t('onb.terms.dataDir'), <Code>{s.settings.dataDir}</Code>],
              [t('profile.label'), t(`profile.${s.resources.profile}` as TKey)],
              [t('folders.sources'), s.settings.videoSourceDirs.length ? s.settings.videoSourceDirs.join(', ') : t('folders.none')],
              [t('folders.output'), s.settings.outputDir ?? '—'],
              [t('sources.title'), t('onb.review.sources', { n: s.sources.length, skills: s.sources.reduce((a, x) => a + x.skillCount, 0) })],
              [t('nav.tools'), t('onb.review.tools', { n: s.tools.filter((x) => x.installed).length, total: s.tools.length })],
              [t('server.mode'), t(`server.mode.${s.server.mode}` as TKey)],
            ]}
          />
        )}
      </AsyncView>
      <p className="mt-4 text-sm text-muted">{t('onb.review.note')}</p>
      <Button
        className="mt-4"
        variant="primary"
        loading={busy}
        onClick={async () => {
          setBusy(true);
          try {
            await api.onboarding.complete();
            await refresh();
            navigate('/', { replace: true });
          } catch (e) {
            notify.fail(e);
          } finally {
            setBusy(false);
          }
        }}
      >
        {t('onb.finish')}
      </Button>
    </Card>
  );
}
