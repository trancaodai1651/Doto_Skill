import { useEffect, useMemo, useState } from 'react';
import { Link, useParams } from 'react-router';
import { Badge, EmptyState, Select, TextInput, useToast } from '@doto/ui';
import { api } from '../api/client';
import { useServerEvent } from '../api/events';
import type { VideoAsset, VideoKind } from '../api/types';
import { AsyncView, KeyValue, PageHeader } from '../components/common';
import { VideoThumb, videoMeta } from '../components/video';
import { useI18n } from '../i18n';
import { formatBytes, formatDate, formatDuration, formatResolution } from '../lib/format';
import { upsertById } from '../lib/list';
import { useApi } from '../lib/useApi';
import { useCaps } from '../state/session';

type Sort = 'new' | 'old' | 'title' | 'duration';

export function GalleryPage() {
  const { t } = useI18n();
  const { isOwner } = useCaps();
  const { push } = useToast();
  const [q, setQ] = useState('');
  const [debounced, setDebounced] = useState('');
  const [kind, setKind] = useState<VideoKind | ''>(isOwner ? 'export' : '');
  const [sort, setSort] = useState<Sort>('new');
  useEffect(() => {
    const h = setTimeout(() => setDebounced(q.trim()), 250);
    return () => clearTimeout(h);
  }, [q]);
  const videos = useApi(`gallery:${kind}:${debounced}`, () => api.videos.list(kind, debounced || undefined));
  useServerEvent('video', (v) => {
    const isNew = !(videos.data ?? []).some((x) => x.id === v.id);
    if (!kind || v.kind === kind) videos.setData((l) => upsertById(l, v));
    if (isNew && v.status === 'ready') push({ title: t('gallery.newVideo'), description: v.title, tone: 'info' });
  });

  const list = useMemo(() => {
    const l = (videos.data ?? []).slice();
    const needle = q.trim().toLowerCase();
    const f = needle ? l.filter((v) => v.title.toLowerCase().includes(needle) || v.fileName.toLowerCase().includes(needle)) : l;
    switch (sort) {
      case 'old':
        return f.sort((a, b) => a.createdAt.localeCompare(b.createdAt));
      case 'title':
        return f.sort((a, b) => a.title.localeCompare(b.title));
      case 'duration':
        return f.sort((a, b) => (b.durationMs ?? 0) - (a.durationMs ?? 0));
      default:
        return f.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    }
  }, [videos.data, q, sort]);

  return (
    <>
      <PageHeader title={t('nav.gallery')} description={t('gallery.desc')} />
      <div className="mb-4 grid gap-3 sm:grid-cols-3">
        <TextInput label={t('common.search')} type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('gallery.searchPlaceholder')} />
        {isOwner && (
          <Select
            label={t('gallery.kind')}
            value={kind}
            onChange={(e) => setKind(e.target.value as VideoKind | '')}
            options={[
              { value: 'export', label: t('videos.exports') },
              { value: 'source', label: t('videos.sources') },
              { value: '', label: t('common.all') },
            ]}
          />
        )}
        <Select
          label={t('gallery.sort')}
          value={sort}
          onChange={(e) => setSort(e.target.value as Sort)}
          options={[
            { value: 'new', label: t('gallery.sort.new') },
            { value: 'old', label: t('gallery.sort.old') },
            { value: 'title', label: t('gallery.sort.title') },
            { value: 'duration', label: t('gallery.sort.duration') },
          ]}
        />
      </div>
      <AsyncView
        state={videos}
        isEmpty={() => list.length === 0}
        empty={<EmptyState icon="▦" title={q ? t('videos.noMatch') : t('gallery.empty')} description={q ? undefined : t('gallery.emptyDesc')} />}
      >
        {() => (
          <ul className="grid grid-cols-1 gap-4 min-[480px]:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4" data-testid="gallery-grid">
            {list.map((v) => (
              <li key={v.id}>
                <Link to={`/gallery/${v.id}`} className="group block rounded-lg border border-line bg-surface p-2 hover:border-primary">
                  <VideoThumb video={v} />
                  <p className="mt-2 truncate font-semibold group-hover:text-primary" title={v.title}>
                    {v.title}
                  </p>
                  <p className="text-xs text-muted">{videoMeta(v)}</p>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </AsyncView>
    </>
  );
}

export function PlayerPage() {
  const { id = '' } = useParams();
  const { t, lang } = useI18n();
  const video = useApi(`video:${id}`, () => api.videos.get(id));
  return (
    <>
      <p className="mb-3">
        <Link to="/gallery" className="text-sm text-primary underline">
          ← {t('gallery.back')}
        </Link>
      </p>
      <AsyncView state={video}>
        {(v: VideoAsset) => (
          <article>
            <h1 className="mb-3 text-xl font-bold break-words md:text-2xl">{v.title}</h1>
            <div className="overflow-hidden rounded-lg bg-black">
              <video
                key={v.id}
                src={api.videos.streamUrl(v.id)}
                controls
                playsInline
                preload="metadata"
                poster={v.hasThumbnail ? api.videos.thumbnailUrl(v.id) : undefined}
                className="aspect-video max-h-[75dvh] w-full"
                data-testid="video-player"
              >
                {t('player.unsupported')}
              </video>
            </div>
            <div className="mt-4 flex flex-wrap items-center gap-3">
              <a href={api.videos.downloadUrl(v.id)} download={v.fileName} className="ui-btn ui-btn--primary" data-testid="download-link">
                ⬇ {t('player.download')}
              </a>
              {v.status !== 'ready' && <Badge tone="danger">{t(`video.status.${v.status}`)}</Badge>}
            </div>
            <div className="mt-4 rounded-lg border border-line bg-surface p-4">
              <KeyValue
                items={[
                  [t('player.duration'), formatDuration(v.durationMs)],
                  [t('player.resolution'), formatResolution(v.width, v.height)],
                  [t('player.size'), formatBytes(v.sizeBytes)],
                  [t('player.codec'), [v.videoCodec, v.audioCodec].filter(Boolean).join(' / ') || '—'],
                  [t('player.created'), formatDate(v.createdAt, lang)],
                  [t('player.file'), v.fileName],
                ]}
              />
            </div>
          </article>
        )}
      </AsyncView>
    </>
  );
}
