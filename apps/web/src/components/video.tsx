import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import { Button, Card, ConfirmDialog, Select, useToast } from '@doto/ui';
import { api, http } from '../api/client';
import type { ShareLink, UploadedMedia, VideoAsset } from '../api/types';
import { useNotify } from './common';
import { useI18n, useT } from '../i18n';
import { formatBytes, formatDate, formatDuration, formatResolution } from '../lib/format';
import { useApi } from '../lib/useApi';

export function VideoThumb({ video, className }: { video: VideoAsset; className?: string }) {
  const [failed, setFailed] = useState(false);
  const t = useT();
  return (
    <div className={`relative aspect-video overflow-hidden rounded-md bg-surface-2 ${className ?? ''}`}>
      {video.hasThumbnail && !failed ? (
        <img
          src={api.videos.thumbnailUrl(video.id)}
          alt=""
          loading="lazy"
          decoding="async"
          className="h-full w-full object-cover"
          onError={() => setFailed(true)}
        />
      ) : (
        <div className="flex h-full w-full items-center justify-center text-3xl text-muted" aria-hidden="true">
          ▶
        </div>
      )}
      {video.durationMs != null && (
        <span className="absolute right-1.5 bottom-1.5 rounded bg-black/75 px-1.5 py-0.5 font-mono text-xs text-white">
          {formatDuration(video.durationMs)}
        </span>
      )}
      {video.status !== 'ready' && (
        <span className="absolute top-1.5 left-1.5 rounded bg-black/75 px-1.5 py-0.5 text-xs text-white">{t(`video.status.${video.status}`)}</span>
      )}
    </div>
  );
}

export function videoMeta(v: VideoAsset): string {
  return [formatResolution(v.width, v.height), formatBytes(v.sizeBytes), v.videoCodec].filter((x) => x && x !== '—').join(' · ');
}

export function MobileUploadCard({ onUploaded }: { onUploaded: () => void }) {
  const { t } = useI18n();
  const notify = useNotify();
  const uploads = useApi('media-uploads', () => api.uploads.list());
  const [uploading, setUploading] = useState(false);

  return (
    <Card title={t('uploads.title')} description={t('uploads.desc')} className="mb-5">
      <div className="flex flex-wrap items-center gap-3">
        <label className="ui-btn ui-btn--primary cursor-pointer" data-testid="mobile-upload-label">
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
                for (const f of files) {
                  await api.uploads.upload(f);
                }
                notify.ok(t('uploads.uploadedCount', { n: files.length }));
                await uploads.reload();
                onUploaded();
              } catch (ex) {
                notify.fail(ex);
              } finally {
                setUploading(false);
              }
            }}
          />
        </label>
      </div>
      {(uploads.data?.length ?? 0) > 0 && (
        <details className="mt-3">
          <summary className="cursor-pointer text-xs font-semibold text-muted">
            {t('uploads.recent', { n: uploads.data?.length ?? 0 })}
          </summary>
          <ul className="mt-2 divide-y divide-line text-xs">
            {(uploads.data ?? []).slice(0, 12).map((u: UploadedMedia) => (
              <li key={u.id} className="flex flex-wrap items-center justify-between gap-2 py-1.5">
                <span>
                  {u.mediaKind === 'video' ? '🎬' : '🖼'} <strong>{u.originalName}</strong> · {formatBytes(u.sizeBytes)}
                  {u.videoId && (
                    <>
                      {' · '}
                      <Link to={`/gallery/${u.videoId}`} className="text-primary underline">
                        {t('nav.gallery')}
                      </Link>
                    </>
                  )}
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={async () => {
                    try {
                      await api.uploads.remove(u.id);
                      await uploads.reload();
                      onUploaded();
                    } catch (ex) {
                      notify.fail(ex);
                    }
                  }}
                >
                  {t('common.delete')}
                </Button>
              </li>
            ))}
          </ul>
        </details>
      )}
    </Card>
  );
}

export function SaveToPhotosButton({ video }: { video: VideoAsset }) {
  const { t } = useI18n();
  const notify = useNotify();
  const [busy, setBusy] = useState(false);

  return (
    <Button
      variant="secondary"
      loading={busy}
      data-testid="save-photos-btn"
      onClick={async () => {
        setBusy(true);
        try {
          const url = api.videos.downloadUrl(video.id);
          const headers: Record<string, string> = {};
          const bearer = http.getBearerToken();
          if (bearer) headers['Authorization'] = `Bearer ${bearer}`;
          const res = await fetch(url, {
            headers,
            credentials: http.isCrossOrigin() || bearer ? 'omit' : 'include',
          });
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
          const blob = await res.blob();
          const safeName = video.fileName.toLowerCase().endsWith('.mp4') ? video.fileName : `${video.title || 'video'}.mp4`;
          const file = new File([blob], safeName, { type: 'video/mp4' });
          const nav = navigator as Navigator & {
            canShare?: (data?: ShareData) => boolean;
          };
          if (typeof nav.share === 'function' && typeof nav.canShare === 'function' && nav.canShare({ files: [file] })) {
            await nav.share({ files: [file], title: video.title });
          } else {
            const objUrl = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = objUrl;
            a.download = safeName;
            document.body.appendChild(a);
            a.click();
            a.remove();
            setTimeout(() => URL.revokeObjectURL(objUrl), 5000);
            notify.info(t('player.savePhotos'), t('player.shareFallback'));
          }
        } catch (e) {
          if (e instanceof Error && e.name === 'AbortError') return;
          notify.fail(e);
        } finally {
          setBusy(false);
        }
      }}
    >
      📲 {t('player.savePhotos')}
    </Button>
  );
}

const SHARE_TTLS = [60, 24 * 60, 7 * 24 * 60];

/** Owner tạo/thu hồi link chia sẻ có hạn cho video đã xuất (API: GET/POST /api/videos/:id/shares|share, DELETE /api/shares/:id). */
export function SharePanel({ videoId }: { videoId: string }) {
  const { t, lang } = useI18n();
  const toast = useToast();
  const [ttl, setTtl] = useState(String(SHARE_TTLS[0]));
  const [confirm, setConfirm] = useState(false);
  const [links, setLinks] = useState<ShareLink[]>([]);
  useEffect(() => {
    let alive = true;
    api.videos
      .shares(videoId)
      .then((list) => {
        if (alive && Array.isArray(list)) setLinks(list);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [videoId]);
  const label = (m: number) => (m >= 1440 ? t('share.days', { n: m / 1440 }) : m >= 60 ? t('share.hours', { n: m / 60 }) : t('share.minutes', { n: m }));

  async function copy(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      toast.push({ tone: 'success', title: t('share.copied') });
    } catch {
      window.prompt(t('share.copy'), url);
    }
  }

  return (
    <section className="mt-4 rounded-lg border border-line bg-surface p-4" aria-labelledby="share-title" data-testid="share-panel">
      <h2 id="share-title" className="font-semibold">
        {t('share.title')}
      </h2>
      <p className="mt-1 text-sm text-muted">{t('share.desc')}</p>
      <div className="mt-3 flex flex-wrap items-end gap-2">
        <Select
          label={t('share.ttl')}
          value={ttl}
          onChange={(e) => setTtl(e.target.value)}
          options={SHARE_TTLS.map((m) => ({ value: String(m), label: label(m) }))}
        />
        <Button onClick={() => setConfirm(true)}>{t('share.create')}</Button>
      </div>
      {links.length > 0 && (
        <ul className="mt-3 flex flex-col gap-2">
          {links.map((l) => (
            <li key={l.id} className="flex flex-wrap items-center gap-2 rounded-md bg-surface-2 p-2 text-sm">
              <code className="min-w-0 flex-1 break-all">{l.url ?? `Link #${l.id.slice(0, 8)} (${t('share.hiddenToken')})`}</code>
              <span className="text-muted">
                {t('share.expires')}: {formatDate(l.expiresAt, lang)}
              </span>
              {l.url && (
                <Button size="sm" onClick={() => void copy(l.url!)}>
                  {t('share.copy')}
                </Button>
              )}
              <Button
                size="sm"
                variant="danger"
                onClick={async () => {
                  await api.videos.revokeShare(l.id);
                  setLinks((xs) => xs.filter((x) => x.id !== l.id));
                  toast.push({ tone: 'success', title: t('share.revoked') });
                }}
              >
                {t('share.revoke')}
              </Button>
            </li>
          ))}
        </ul>
      )}
      <ConfirmDialog
        open={confirm}
        title={t('share.confirmTitle')}
        confirmLabel={t('share.create')}
        onCancel={() => setConfirm(false)}
        onConfirm={async () => {
          const link = await api.videos.share(videoId, Number(ttl));
          setLinks((xs) => [link, ...xs]);
          setConfirm(false);
          if (link.url) await copy(link.url);
        }}
      >
        <p>{t('share.confirmBody', { ttl: label(Number(ttl)) })}</p>
      </ConfirmDialog>
    </section>
  );
}
