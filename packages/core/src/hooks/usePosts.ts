import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useSiteConfig } from '../context';
import type { PostMetadata } from '../types/blog';
import { resolveSitePath } from '../utils/sitePath';

interface UsePostsResult {
  posts: PostMetadata[];
  loading: boolean;
  error: string | null;
}

// Module-level cache: all usePosts() callers share the same fetch
let cachedPromise: Promise<PostMetadata[]> | null = null;
let cachedData: PostMetadata[] | null = null;
let cachedError: string | null = null;
let cachedManifestUrl: string | null = null;

function fetchManifest(manifestUrl: string): Promise<PostMetadata[]> {
  if (cachedManifestUrl !== manifestUrl) {
    cachedManifestUrl = manifestUrl;
    cachedPromise = null;
    cachedData = null;
    cachedError = null;
  }
  if (!cachedPromise) {
    cachedError = null;
    cachedPromise = fetch(manifestUrl, { cache: 'no-cache' })
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Failed to load posts manifest: ${response.status}`);
        }
        return response.json() as Promise<PostMetadata[]>;
      })
      .then((data) => {
        cachedData = data;
        cachedError = null;
        return data;
      })
      .catch((err) => {
        cachedError = err instanceof Error ? err.message : 'Failed to load posts';
        cachedPromise = null; // allow retry on error
        throw err;
      });
  }
  return cachedPromise;
}

export function usePosts(): UsePostsResult {
  const siteConfig = useSiteConfig();
  const manifestUrl = resolveSitePath('/generated/manifest.json', siteConfig.basePath);
  const hasCurrentCache = cachedManifestUrl === manifestUrl;
  const [rawPosts, setRawPosts] = useState<PostMetadata[]>(hasCurrentCache ? cachedData ?? [] : []);
  const [loading, setLoading] = useState(!hasCurrentCache || cachedData === null);
  const [error, setError] = useState<string | null>(hasCurrentCache ? cachedError : null);
  const { i18n } = useTranslation();
  const currentLang = i18n.resolvedLanguage ?? '';

  useEffect(() => {
    if (cachedManifestUrl === manifestUrl && cachedData) {
      // Already have data, no fetch needed
      setRawPosts(cachedData);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    fetchManifest(manifestUrl)
      .then((data) => {
        if (!cancelled) {
          setRawPosts(data);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load posts');
          setLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [manifestUrl]);

  // Apply localized title/summary/tags/categories based on current language
  const posts = useMemo(() => {
    if (!currentLang) return rawPosts;
    return rawPosts.map((post) => {
      const localized = post.localizedMeta?.[currentLang];
      if (localized) {
        return {
          ...post,
          title: localized.title,
          summary: localized.summary,
          ...(localized.tags && localized.tags.length > 0 ? { tags: localized.tags } : {}),
          ...(localized.categories && localized.categories.length > 0 ? { categories: localized.categories } : {}),
        };
      }
      return post;
    });
  }, [rawPosts, currentLang]);

  return { posts, loading, error };
}
