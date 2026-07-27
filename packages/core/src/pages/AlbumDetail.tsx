import React, { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useAlbum } from '../hooks/useAlbum';
import PhotoViewer from '../components/PhotoViewer';
import { PhotoGridSkeleton } from '../components/Skeleton';
import { useSignalReady } from '../AppReadyProvider';

const AlbumDetail: React.FC = () => {
  const { dirname } = useParams<{ dirname: string }>();
  const { album, loading, error } = useAlbum(dirname || '');
  const [viewerIndex, setViewerIndex] = useState<number | null>(null);
  const { t } = useTranslation();

  useSignalReady(!loading);

  if (loading) {
    return (
      <div>
        <div className="mb-6">
          <Link to="/albums" className="text-accent hover:underline text-sm">
            {t('albums.backToAlbums')}
          </Link>
        </div>
        <PhotoGridSkeleton />
      </div>
    );
  }

  if (error || !album) {
    return (
      <div className="text-center py-12">
        <div className="mb-6">
          <Link to="/albums" className="text-accent hover:underline text-sm">
            {t('albums.backToAlbums')}
          </Link>
        </div>
        <p className="text-secondary">{error || t('albums.albumNotFound')}</p>
      </div>
    );
  }

  return (
    <div>
      <div className="mb-6">
        <Link to="/albums" className="text-accent hover:underline text-sm">
          {t('albums.backToAlbums')}
        </Link>
        <h2 className="text-2xl font-bold mt-2 mb-1">{album.name}</h2>
        {album.desc && (
          <p className="text-secondary whitespace-pre-wrap mt-3 mb-2">{album.desc}</p>
        )}
        <p className="text-secondary text-sm m-0">{t('albums.photoCount', { count: album.photos.length })}</p>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3 md:gap-4">
        {album.photos.map((photo, index) => (
          <button
            key={photo.filename}
            onClick={() => setViewerIndex(index)}
            className="aspect-square overflow-hidden rounded-lg cursor-pointer p-0 border-0"
            style={{
              background: 'var(--color-bg-alt)',
              transition: 'transform 0.2s ease, box-shadow 0.2s ease',
            }}
            onMouseEnter={(e) => {
              (e.currentTarget as HTMLElement).style.transform = 'scale(1.02)';
              (e.currentTarget as HTMLElement).style.boxShadow = '0 4px 16px rgba(0,0,0,0.1)';
            }}
            onMouseLeave={(e) => {
              (e.currentTarget as HTMLElement).style.transform = '';
              (e.currentTarget as HTMLElement).style.boxShadow = '';
            }}
          >
            <img
              src={photo.thumbnailUrl}
              alt={photo.filename}
              loading="lazy"
              className="w-full h-full object-cover"
              style={{ transition: 'transform 0.3s ease' }}
            />
          </button>
        ))}
      </div>

      {viewerIndex !== null && (
        <PhotoViewer
          photos={album.photos}
          initialIndex={viewerIndex}
          onClose={() => setViewerIndex(null)}
        />
      )}
    </div>
  );
};

export default AlbumDetail;
