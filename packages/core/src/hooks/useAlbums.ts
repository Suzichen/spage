import { useState, useEffect } from 'react';
import type { AlbumSummary } from '../types/album';
import { useSiteConfig } from '../context';
import { resolveSitePath } from '../utils/sitePath';

interface UseAlbumsResult {
  albums: AlbumSummary[];
  loading: boolean;
  error: string | null;
}

export function useAlbums(): UseAlbumsResult {
  const siteConfig = useSiteConfig();
  const [albums, setAlbums] = useState<AlbumSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function fetchAlbums() {
      try {
        const url = resolveSitePath('/generated/albums-index.json', siteConfig.basePath);
        const response = await fetch(url, { cache: 'no-cache' });
        if (!response.ok) {
          throw new Error(`Failed to load albums index: ${response.status}`);
        }
        const data: AlbumSummary[] = await response.json();
        if (!cancelled) {
          setAlbums(data);
          setLoading(false);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load albums');
          setLoading(false);
        }
      }
    }

    fetchAlbums();
    return () => { cancelled = true; };
  }, [siteConfig.basePath]);

  return { albums, loading, error };
}
