import { useState } from 'react';
import { api } from '../api/client';
import type { VideoAsset } from '../api/types';
import { useT } from '../i18n';
import { formatBytes, formatDuration, formatResolution } from '../lib/format';

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
