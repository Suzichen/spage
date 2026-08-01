import { useEffect, useMemo, useRef, useState, type PropsWithChildren } from 'react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import { usePosts } from '../hooks/usePosts';
import { useSiteConfig } from '../context';
import {
  normalizeSupportedLanguage,
  parseLanguageEntry,
  resolveDefaultLanguage,
  resolveInitialLanguage,
  resolveLanguagePath,
  resolvePostSlug,
} from '../utils/languageEntry';

const LanguagePathSync = ({ children }: PropsWithChildren) => {
  const { i18n } = useTranslation();
  const { pathname, search, hash } = useLocation();
  const navigate = useNavigate();
  const siteConfig = useSiteConfig();
  const { posts, loading } = usePosts();
  const [ready, setReady] = useState(false);
  const handledEntryPath = useRef<string | null>(null);
  const defaultLanguage = useMemo(
    () => resolveDefaultLanguage(siteConfig.language),
    [siteConfig.language],
  );
  const currentLanguage = normalizeSupportedLanguage(i18n.resolvedLanguage);
  const entry = useMemo(() => parseLanguageEntry(pathname), [pathname]);
  const postSlug = resolvePostSlug(pathname);
  const postLanguages = postSlug
    ? posts.find((candidate) => candidate.slug === postSlug)?.availableLanguages
    : undefined;

  useEffect(() => {
    let storedLanguage = null;
    try {
      storedLanguage = window.localStorage.getItem('i18nextLng');
    } catch {}
    let cancelled = false;
    const finish = () => {
      if (!cancelled) setReady(true);
    };
    void i18n.changeLanguage(resolveInitialLanguage(
      window.location.pathname,
      storedLanguage,
      defaultLanguage,
    )).then(finish, finish);

    return () => {
      cancelled = true;
    };
  }, [defaultLanguage, i18n]);

  useEffect(() => {
    if (ready && entry && !entry.language) {
      navigate({ pathname: entry.destination, search, hash }, { replace: true });
    }
  }, [entry, hash, navigate, ready, search]);

  // A language path is an explicit user choice. Consume and persist it once;
  // later menu changes on the same URL must remain authoritative.
  useEffect(() => {
    if (!ready) return;
    if (!entry || handledEntryPath.current === pathname) return;
    if (!entry.language) return;
    if (postSlug && loading) return;

    handledEntryPath.current = pathname;
    try {
      window.localStorage.setItem('i18nextLng', entry.language);
    } catch {}
    if (currentLanguage !== entry.language) void i18n.changeLanguage(entry.language);
  }, [currentLanguage, entry, i18n, loading, pathname, postSlug, ready]);

  // Keep the URL canonical after initialization or a language-menu change.
  useEffect(() => {
    if (!ready || !currentLanguage || (postSlug && loading)) return;
    if (entry && handledEntryPath.current !== pathname) return;
    if (!entry) handledEntryPath.current = null;

    const targetPath = resolveLanguagePath({
      pathname,
      language: currentLanguage,
      defaultLanguage,
      availableLanguages: postLanguages,
    });
    if (targetPath !== pathname) {
      navigate({ pathname: targetPath, search, hash }, { replace: true });
    }
  }, [currentLanguage, defaultLanguage, entry, hash, loading, navigate, pathname, postLanguages, postSlug, ready, search]);

  // The static skeleton is mounted in a separate root, so this gate prevents
  // wrong-language content from loading without leaving the first paint blank.
  return ready ? <>{children}</> : null;
};

export default LanguagePathSync;
