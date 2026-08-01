import React from 'react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import { usePosts } from '../hooks/usePosts';
import { useSiteConfig } from '../context';
import {
  normalizeSupportedLanguage,
  resolveDefaultLanguage,
  resolveInitialLanguage,
  resolveLanguageEntryDestination,
  resolveLanguagePath,
  resolvePostSlug,
} from '../utils/languageEntry';

const LANGUAGE_ENTRY_PATTERN = /\/lang\/([^/]+)\/?$/;

const LanguagePathSync: React.FC<React.PropsWithChildren> = ({ children }) => {
  const { i18n } = useTranslation();
  const { pathname, search, hash } = useLocation();
  const navigate = useNavigate();
  const siteConfig = useSiteConfig();
  const { posts, loading } = usePosts();
  const [ready, setReady] = React.useState(false);
  const handledEntryPath = React.useRef<string | null>(null);

  React.useEffect(() => {
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
      resolveDefaultLanguage(siteConfig.language),
    )).then(finish, finish);

    return () => {
      cancelled = true;
    };
  }, [i18n, siteConfig.language]);

  React.useEffect(() => {
    if (!ready) return;
    const entryMatch = pathname.match(LANGUAGE_ENTRY_PATTERN);
    const entryLanguage = normalizeSupportedLanguage(entryMatch?.[1]);
    if (entryMatch && !entryLanguage) {
      navigate({ pathname: resolveLanguageEntryDestination(pathname), search, hash }, { replace: true });
      return;
    }

    const newEntryLanguage = entryLanguage && handledEntryPath.current !== pathname
      ? entryLanguage
      : null;
    const language = newEntryLanguage ?? normalizeSupportedLanguage(i18n.resolvedLanguage);
    if (!language) return;

    const postSlug = resolvePostSlug(pathname);
    if (postSlug && loading) return;
    if (newEntryLanguage) handledEntryPath.current = pathname;
    if (!entryLanguage) handledEntryPath.current = null;
    const post = postSlug ? posts.find((candidate) => candidate.slug === postSlug) : undefined;
    let cancelled = false;
    const syncPath = () => {
      if (cancelled) return;
      const targetPath = resolveLanguagePath({
        pathname,
        language,
        defaultLanguage: resolveDefaultLanguage(siteConfig.language),
        availableLanguages: post?.availableLanguages,
      });
      if (targetPath !== pathname) {
        navigate({ pathname: targetPath, search, hash }, { replace: true });
      }
    };

    if (!newEntryLanguage) {
      syncPath();
      return;
    }

    try {
      window.localStorage.setItem('i18nextLng', newEntryLanguage);
    } catch {}
    if (normalizeSupportedLanguage(i18n.resolvedLanguage) === newEntryLanguage) {
      syncPath();
    } else {
      void i18n.changeLanguage(newEntryLanguage).then(syncPath, syncPath);
    }

    return () => {
      cancelled = true;
    };
  }, [hash, i18n, i18n.resolvedLanguage, loading, navigate, pathname, posts, ready, search, siteConfig.language]);

  return ready ? <>{children}</> : null;
};

export default LanguagePathSync;
