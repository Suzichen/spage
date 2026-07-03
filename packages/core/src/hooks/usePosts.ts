import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { PostMetadata } from '../types/blog';

interface UsePostsResult {
  posts: PostMetadata[];
  loading: boolean;
  error: string | null;
}

// Module-level cache: all usePosts() callers share the same fetch
let cachedPromise: Promise<PostMetadata[]> | null = null;
let cachedData: PostMetadata[] | null = null;
let cachedError: string | null = null;

function fetchManifest(): Promise<PostMetadata[]> {
  if (!cachedPromise) {
    cachedError = null;
    cachedPromise = fetch('/generated/manifest.json', { cache: 'no-cache' })
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
  const [rawPosts, setRawPosts] = useState<PostMetadata[]>(cachedData ?? []);
  const [loading, setLoading] = useState(cachedData === null);
  const [error, setError] = useState<string | null>(cachedError);
  const { i18n } = useTranslation();
  const currentLang = i18n.resolvedLanguage ?? '';

  useEffect(() => {
    if (cachedData) {
      // Already have data, no fetch needed
      setRawPosts(cachedData);
      setLoading(false);
      return;
    }

    let cancelled = false;

    fetchManifest()
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
  }, []);

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
