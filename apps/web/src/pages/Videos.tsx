import { useState } from 'react';
import { Link } from 'react-router';
import { Badge, Button, Card, Checkbox, ConfirmDialog, EmptyState, Select, Tabs, TextInput } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { RenderRequest, VideoAsset, VideoKind } from '../api/types';
import { AsyncView, HostOnly, PageHeader, useNotify } from '../components/common';
import { MobileUploadCard, VideoThumb, videoMeta } from '../components/video';
import { useI18n } from '../i18n';
import { formatDuration, secondsToMs } from '../lib/format';
import { upsertById } from '../lib/list';
import { useApi, useDebouncedValue } from '../lib/useApi';
import { useCaps } from '../state/session';

interface Segment {
  videoId: string;
  start: string;
  end: string;
}

export function VideosPage() {
  const { t } = useI18n();
  const notify = useNotify();
  const [kind, setKind] = useState<VideoKind>('source');
  const [q, setQ] = useState('');
  const debounced = useDebouncedValue(q.trim());
  const [view, setView] = useState<'grid' | 'list'>('grid');
  const [segments, setSegments] = useState<Segment[]>([]);
  const [scanning, setScanning] = useState(false);
  const videos = useApi(`videos:${kind}:${debounced}`, () => api.videos.list(kind, debounced || undefined));
  useServerEvent('video', (v) => {
    if (v.kind === kind) videos.setData((l) => upsertById(l, v));
  });

  const selected = (id: string) => segments.some((s) => s.videoId === id);
  const toggle = (id: string) =>
    setSegments((s) => (s.some((x) => x.videoId === id) ? s.filter((x) => x.videoId !== id) : [...s, { videoId: id, start: '', end: '' }]));

  return (
    <>
      <PageHeader
        title={t('nav.videos')}
        description={t('videos.desc')}
        actions={
          <>
            <Button size="sm" onClick={() => setView(view === 'grid' ? 'list' : 'grid')} aria-pressed={view === 'list'}>
              {view === 'grid' ? t('videos.listView') : t('videos.gridView')}
            </Button>
            <HostOnly>
              <Button
                size="sm"
                variant="primary"
                loading={scanning}
                onClick={async () => {
                  setScanning(true);
                  try {
                    await api.videos.scan();
                    notify.ok(t('videos.scanStarted'));
                    await videos.reload();
                  } catch (e) {
                    notify.fail(e);
                  } finally {
                    setScanning(false);
                  }
                }}
              >
                {t('videos.scan')}
              </Button>
            </HostOnly>
          </>
        }
      />
      <HostOnly>
        <MobileUploadCard onUploaded={() => void videos.reload()} />
      </HostOnly>
      <Tabs
        label={t('nav.videos')}
        value={kind}
        onChange={(k) => setKind(k as VideoKind)}
        tabs={[
          { id: 'source', label: t('videos.sources') },
          { id: 'export', label: t('videos.exports') },
        ]}
      >
        <div className="mb-4 max-w-md">
          <TextInput label={t('common.search')} type="search" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <AsyncView
          state={videos}
          isEmpty={(l) => l.length === 0}
          empty={
            <EmptyState
              icon="▶"
              title={debounced ? t('videos.noMatch') : kind === 'source' ? t('videos.emptySource') : t('videos.emptyExport')}
              description={!debounced && kind === 'source' ? t('videos.emptySourceDesc') : undefined}
            />
          }
        >
          {(list) =>
            view === 'grid' ? (
              <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3" data-testid="videos-grid">
                {list.map((v) => (
                  <VideoCard key={v.id} video={v} selectable={kind === 'source'} selected={selected(v.id)} onToggle={() => toggle(v.id)} onChanged={() => void videos.reload()} />
                ))}
              </ul>
            ) : (
              <ul className="flex flex-col gap-2">
                {list.map((v) => (
                  <VideoCard key={v.id} compact video={v} selectable={kind === 'source'} selected={selected(v.id)} onToggle={() => toggle(v.id)} onChanged={() => void videos.reload()} />
                ))}
              </ul>
            )
          }
        </AsyncView>
      </Tabs>
      {segments.length > 0 && <RenderPanel segments={segments} setSegments={setSegments} videos={videos.data ?? []} />}
    </>
  );
}

function VideoCard({
  video: v,
  selectable,
  selected,
  onToggle,
  onChanged,
  compact,
}: {
  video: VideoAsset;
  selectable: boolean;
  selected: boolean;
  onToggle: () => void;
  onChanged: () => void;
  compact?: boolean;
}) {
  const { t } = useI18n();
  const notify = useNotify();
  const { hostControl } = useCaps();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleteFile, setDeleteFile] = useState(false);
  const [busy, setBusy] = useState(false);

  return (
    <li className={`rounded-lg border bg-surface p-3 ${selected ? 'border-primary' : 'border-line'} ${compact ? 'flex flex-wrap items-center gap-3' : ''}`}>
      <Link to={`/gallery/${v.id}`} className={compact ? 'w-32 shrink-0' : 'block'} aria-label={t('videos.open', { title: v.title })}>
        <VideoThumb video={v} />
      </Link>
      <div className={compact ? 'min-w-0 flex-1' : 'mt-2'}>
        <p className="truncate font-semibold" title={v.title}>
          {v.title}
        </p>
        <p className="text-xs text-muted">{videoMeta(v)}</p>
        <div className="mt-1 flex flex-wrap gap-1">
          <Badge tone={v.visibility === 'viewers' ? 'success' : 'neutral'}>{v.visibility === 'viewers' ? t('videos.visibleViewers') : t('videos.private')}</Badge>
          {v.status !== 'ready' && <Badge tone="danger">{t(`video.status.${v.status}`)}</Badge>}
        </div>
      </div>
      <div className={`flex flex-wrap items-center gap-2 ${compact ? '' : 'mt-2'}`}>
        {selectable && <Checkbox label={t('videos.select')} checked={selected} onChange={onToggle} />}
        {v.kind === 'export' && (
          <Button
            size="sm"
            loading={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await api.videos.patch(v.id, { visibility: v.visibility === 'viewers' ? 'private' : 'viewers' });
                onChanged();
              } catch (e) {
                notify.fail(e);
              } finally {
                setBusy(false);
              }
            }}
          >
            {v.visibility === 'viewers' ? t('videos.makePrivate') : t('videos.shareViewers')}
          </Button>
        )}
        <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(true)} aria-label={t('videos.deleteNamed', { title: v.title })}>
          {t('common.delete')}
        </Button>
      </div>
      <ConfirmDialog
        open={confirmDelete}
        tone="danger"
        title={t('videos.deleteTitle', { title: v.title })}
        confirmLabel={t('common.delete')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={async () => {
          await api.videos.remove(v.id, deleteFile && hostControl && v.kind === 'export');
          setConfirmDelete(false);
          notify.ok(t('videos.deleted'));
          onChanged();
        }}
      >
        <p>{t('videos.deleteBody')}</p>
        {v.kind === 'export' && hostControl && (
          <div className="mt-3">
            <Checkbox label={t('videos.deleteFile')} checked={deleteFile} onChange={(e) => setDeleteFile(e.target.checked)} />
          </div>
        )}
      </ConfirmDialog>
    </li>
  );
}

function RenderPanel({ segments, setSegments, videos }: { segments: Segment[]; setSegments: (s: Segment[]) => void; videos: VideoAsset[] }) {
  const { t } = useI18n();
  const notify = useNotify();
  const [title, setTitle] = useState('');
  const [encoder, setEncoder] = useState('auto');
  const [preset, setPreset] = useState('');
  const [crf, setCrf] = useState('');
  const [visibility, setVisibility] = useState<'private' | 'viewers'>('private');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const byId = new Map(videos.map((v) => [v.id, v]));

  function update(i: number, patch: Partial<Segment>) {
    setSegments(segments.map((s, j) => (j === i ? { ...s, ...patch } : s)));
  }
  function move(i: number, d: -1 | 1) {
    const j = i + d;
    if (j < 0 || j >= segments.length) return;
    const copy = segments.slice();
    [copy[i], copy[j]] = [copy[j]!, copy[i]!];
    setSegments(copy);
  }

  async function submit() {
    const errs: Record<string, string> = {};
    if (!title.trim()) errs.title = t('render.titleRequired');
    const segs = segments.map((s, i) => {
      const startMs = secondsToMs(s.start);
      const endMs = secondsToMs(s.end);
      if (Number.isNaN(startMs)) errs[`s${i}`] = t('render.badNumber');
      if (Number.isNaN(endMs)) errs[`e${i}`] = t('render.badNumber');
      if (startMs != null && endMs != null && !Number.isNaN(startMs) && !Number.isNaN(endMs) && endMs <= startMs) errs[`e${i}`] = t('render.endAfterStart');
      return { videoId: s.videoId, startMs, endMs };
    });
    const crfN = crf.trim() ? Number(crf) : null;
    if (crfN != null && (!Number.isInteger(crfN) || crfN < 18 || crfN > 30)) errs.crf = t('render.crfRange');
    setErrors(errs);
    if (Object.keys(errs).length) return;
    const req: RenderRequest = {
      title: title.trim(),
      segments: segs,
      encoder,
      preset: preset || null,
      crf: crfN,
      visibility,
    };
    setBusy(true);
    try {
      const job = await api.jobs.render(req);
      notify.ok(t('render.started'), job.title);
      setSegments([]);
    } catch (e) {
      notify.fail(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card title={t('render.title')} description={t('render.desc')} className="mt-6">
      <ol className="flex flex-col gap-3">
        {segments.map((s, i) => {
          const v = byId.get(s.videoId);
          return (
            <li key={s.videoId} className="rounded-md border border-line p-3">
              <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
                <span className="font-medium">
                  {i + 1}. {v?.title ?? s.videoId} <span className="text-xs text-muted">({formatDuration(v?.durationMs)})</span>
                </span>
                <span className="flex gap-1">
                  <Button size="sm" variant="ghost" onClick={() => move(i, -1)} disabled={i === 0} aria-label={t('render.moveUp')}>
                    ↑
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => move(i, 1)} disabled={i === segments.length - 1} aria-label={t('render.moveDown')}>
                    ↓
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setSegments(segments.filter((_, j) => j !== i))} aria-label={t('render.removeSegment')}>
                    ✕
                  </Button>
                </span>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <TextInput label={t('render.start')} inputMode="decimal" placeholder="0" value={s.start} onChange={(e) => update(i, { start: e.target.value })} error={errors[`s${i}`]} />
                <TextInput label={t('render.end')} inputMode="decimal" placeholder={t('render.endPlaceholder')} value={s.end} onChange={(e) => update(i, { end: e.target.value })} error={errors[`e${i}`]} />
              </div>
            </li>
          );
        })}
      </ol>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <TextInput label={t('render.outputTitle')} value={title} onChange={(e) => setTitle(e.target.value)} error={errors.title} />
        <Select
          label={t('render.encoder')}
          value={encoder}
          onChange={(e) => setEncoder(e.target.value)}
          options={[
            { value: 'auto', label: t('render.encoderAuto') },
            { value: 'software', label: t('render.encoderSoftware') },
          ]}
        />
        <Select
          label={t('render.preset')}
          value={preset}
          onChange={(e) => setPreset(e.target.value)}
          options={[{ value: '', label: t('render.default') }, ...['veryfast', 'fast', 'medium', 'slow'].map((p) => ({ value: p, label: p }))]}
        />
        <TextInput label="CRF (18–30)" inputMode="numeric" placeholder="23" value={crf} onChange={(e) => setCrf(e.target.value)} error={errors.crf} />
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
      <div className="mt-4 flex gap-2">
        <Button variant="primary" loading={busy} onClick={submit}>
          {t('render.submit')}
        </Button>
        <Button variant="ghost" onClick={() => setSegments([])}>
          {t('render.clear')}
        </Button>
      </div>
    </Card>
  );
}
