import { useState } from 'react';
import { Badge, Button, Card, TextInput, StatusPill } from '@doto/ui';
import { api } from '../api/client';
import { useEvents } from '../api/events';
import type { EncoderBenchmark, ResourceOverrides, ResourceSettings } from '../api/types';
import { AsyncView, Code, HostOnly, KeyValue, PageHeader, Sparkline, useNotify } from '../components/common';
import { LimitsTable, ProfilePicker, useResourceSettings } from '../components/features';
import { useI18n } from '../i18n';
import { formatDate, formatMb, formatPercent } from '../lib/format';
import { useApi } from '../lib/useApi';

export function PerformancePage() {
  const { t } = useI18n();
  return (
    <>
      <PageHeader title={t('nav.performance')} description={t('perf.desc')} />
      <div className="flex flex-col gap-4">
        <LiveCard />
        <ResourcesCard />
        <FfmpegCard />
      </div>
    </>
  );
}

function LiveCard() {
  const { t } = useI18n();
  const { metricsHistory, latestMetrics: m } = useEvents();
  return (
    <Card title={t('perf.live')}>
      {!m ? (
        <p className="text-sm text-muted">{t('perf.waiting')}</p>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          <div>
            <p className="text-sm text-muted">
              CPU · <strong className="text-fg">{formatPercent(m.cpuPercent)}</strong>
            </p>
            <Sparkline values={metricsHistory.map((x) => x.cpuPercent)} max={100} label={t('perf.cpuChart')} />
          </div>
          <div>
            <p className="text-sm text-muted">
              RAM · <strong className="text-fg">{formatMb(m.memUsedMb)}</strong> / {formatMb(m.memTotalMb)}
            </p>
            <Sparkline values={metricsHistory.map((x) => x.memUsedMb)} max={m.memTotalMb} label={t('perf.ramChart')} color="var(--doto-success)" />
          </div>
          {m.throttleReason && <p className="text-sm text-warning md:col-span-2">⚠ {m.throttleReason}</p>}
        </div>
      )}
    </Card>
  );
}

function ResourcesCard() {
  const { t } = useI18n();
  const res = useResourceSettings();
  return (
    <Card title={t('perf.resources')}>
      <AsyncView state={res}>{(r) => <ResourcesForm key={JSON.stringify(r)} settings={r} onSaved={(n) => res.setData(n)} />}</AsyncView>
    </Card>
  );
}

function ResourcesForm({ settings, onSaved }: { settings: ResourceSettings; onSaved: (r: ResourceSettings) => void }) {
  const { t } = useI18n();
  const notify = useNotify();
  const { latestMetrics: m } = useEvents();
  const [busy, setBusy] = useState(false);
  const keys: Array<keyof ResourceOverrides> = ['maxJobs', 'maxCpuThreads', 'maxRamMb', 'maxGpuJobs'];
  const [vals, setVals] = useState<Record<string, string>>(() =>
    Object.fromEntries(keys.map((k) => [k, settings.overrides[k] == null ? '' : String(settings.overrides[k])])),
  );
  const [errs, setErrs] = useState<Record<string, string>>({});

  async function put(profile: ResourceSettings['profile'], overrides: ResourceOverrides) {
    setBusy(true);
    try {
      onSaved(await api.settings.putResources(profile, overrides));
      notify.ok(t('perf.applied'));
    } catch (e) {
      notify.fail(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <ProfilePicker value={settings.profile} busy={busy} onSelect={(p) => void put(p, settings.overrides)} />
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          const next: ResourceOverrides = {};
          const errors: Record<string, string> = {};
          for (const k of keys) {
            const v = vals[k]?.trim() ?? '';
            if (!v) {
              next[k] = null;
              continue;
            }
            const n = Number(v);
            if (!Number.isInteger(n) || n < 0) errors[k] = t('perf.badInt');
            else next[k] = n;
          }
          setErrs(errors);
          if (Object.keys(errors).length) return;
          void put(settings.profile, next);
        }}
      >
        <p className="text-sm font-semibold">{t('perf.overrides')}</p>
        <div className="grid gap-3 sm:grid-cols-4">
          {keys.map((k) => (
            <TextInput
              key={k}
              label={t(`limits.${k}`)}
              inputMode="numeric"
              placeholder={String(settings.effective[k])}
              value={vals[k] ?? ''}
              onChange={(e) => setVals({ ...vals, [k]: e.target.value })}
              error={errs[k]}
            />
          ))}
        </div>
        <p className="text-xs text-muted">{t('perf.overridesHint')}</p>
        <div>
          <Button type="submit" variant="primary" loading={busy}>
            {t('perf.applyOverrides')}
          </Button>
        </div>
      </form>
      <div className="overflow-x-auto">
        <LimitsTable
          limits={settings.effective}
          actual={
            m
              ? {
                  maxJobs: `${m.runningJobs} ${t('perf.running')} · ${m.queuedJobs} ${t('perf.queued')}`,
                  maxRamMb: formatMb(m.memUsedMb),
                  cpuAdmitThreshold: formatPercent(m.cpuPercent),
                  minFreeRamMb: formatMb(m.memAvailableMb),
                }
              : undefined
          }
        />
      </div>
    </div>
  );
}

function FfmpegCard() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const ff = useApi('ffmpeg', () => api.system.ffmpeg());
  const [busy, setBusy] = useState(false);
  const [benchBusy, setBenchBusy] = useState(false);
  const [bench, setBench] = useState<EncoderBenchmark[] | null>(null);
  return (
    <Card
      title="FFmpeg"
      actions={
        <HostOnly>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              loading={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  const r = await api.system.probeFfmpeg();
                  if (r && typeof r === 'object' && 'found' in r) ff.setData(r);
                  else await ff.reload();
                } catch (e) {
                  notify.fail(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t('perf.reprobe')}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              loading={benchBusy}
              disabled={!ff.data?.found}
              onClick={async () => {
                setBenchBusy(true);
                try {
                  const r = await api.system.benchmarkFfmpeg();
                  setBench(r.results);
                  notify.ok(t('perf.benchDone'));
                } catch (e) {
                  notify.fail(e);
                } finally {
                  setBenchBusy(false);
                }
              }}
            >
              {t('perf.benchmark')}
            </Button>
          </div>
        </HostOnly>
      }
    >
      <AsyncView state={ff}>
        {(f) => (
          <div className="flex flex-col gap-3 text-sm">
            <div className="flex items-center gap-2">
              <StatusPill status={f.found ? 'ok' : 'missing'} label={f.found ? t('perf.ffFound') : t('perf.ffMissing')} />
              {f.version && <span className="text-muted">{f.version}</span>}
            </div>
            <KeyValue
              items={[
                ['ffmpeg', f.ffmpegPath ? <Code>{f.ffmpegPath}</Code> : '—'],
                ['ffprobe', f.ffprobePath ? <Code>{f.ffprobePath}</Code> : '—'],
                [t('perf.license'), f.licenseFlags.length ? f.licenseFlags.map((l) => <Badge key={l}>{l}</Badge>) : '—'],
                [t('perf.probedAt'), formatDate(f.encodersProbedAt, lang)],
              ]}
            />
            {f.licenseFlags.some((l) => l === 'nonfree' || l === 'gpl') && (
              <p className="rounded-md bg-warning-bg p-3 text-warning" role="note">
                ⚠ {f.licenseFlags.includes('nonfree') ? t('perf.nonfreeWarning') : t('perf.gplWarning')}
              </p>
            )}
            {!f.found && <p className="text-muted">{t('perf.ffHint')}</p>}
            {f.encoders.length > 0 && (
              <div className="overflow-x-auto">
                <table className="data">
                  <thead>
                    <tr>
                      <th>Encoder</th>
                      <th>{t('perf.encKind')}</th>
                      <th>{t('perf.encAvailable')}</th>
                      <th>{t('perf.encWorking')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {f.encoders.map((e) => (
                      <tr key={e.name}>
                        <td className="font-mono text-xs">
                          {e.name} <span className="text-muted">{e.vendor}</span>
                        </td>
                        <td>{e.kind === 'hardware' ? t('perf.hardware') : t('perf.software')}</td>
                        <td>{e.available ? '✓' : '—'}</td>
                        <td>
                          {e.working ? <Badge tone="success">✓</Badge> : e.error ? <span className="text-xs text-danger">{e.error}</span> : '—'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            {bench && bench.length > 0 && (
              <div className="overflow-x-auto">
                <p className="mb-1 font-semibold">{t('perf.benchTitle')}</p>
                <table className="data" data-testid="benchmark-table">
                  <thead>
                    <tr>
                      <th>Encoder</th>
                      <th>{t('perf.encKind')}</th>
                      <th>FPS (720p)</th>
                      <th>{t('perf.benchTime')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {bench.map((b) => (
                      <tr key={b.name}>
                        <td className="font-mono text-xs">{b.name}</td>
                        <td>{b.kind === 'hardware' ? t('perf.hardware') : t('perf.software')}</td>
                        <td className="font-semibold">{b.fps.toFixed(1)} fps</td>
                        <td>{b.elapsedMs} ms</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}
      </AsyncView>
    </Card>
  );
}
