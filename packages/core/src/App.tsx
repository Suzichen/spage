import React, { Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { useSiteConfig, useAlbumConfig, useMemoConfig } from './context';
import Layout from './components/Layout';
import LanguagePathSync from './components/LanguagePathSync';
import { AppReadyProvider } from './AppReadyProvider';
import { resolveSitePath } from './utils/sitePath';

// Lazy-load route-level pages for code splitting
const Home = React.lazy(() => import('./pages/Home'));
const PostDetail = React.lazy(() => import('./pages/PostDetail'));
const CategoryDetail = React.lazy(() => import('./pages/CategoryDetail'));
const TagDetail = React.lazy(() => import('./pages/TagDetail'));
const Archives = React.lazy(() => import('./pages/Archives'));
const Albums = React.lazy(() => import('./pages/Albums'));
const AlbumDetail = React.lazy(() => import('./pages/AlbumDetail'));
const Memo = React.lazy(() => import('./pages/Memo'));

const App: React.FC = () => {
  const { i18n } = useTranslation();
  const siteConfig = useSiteConfig();
  const albumConfig = useAlbumConfig();
  const memoConfig = useMemoConfig();
  const faviconUrl = resolveSitePath(siteConfig.favicon, siteConfig.basePath);

  // Effect to update lang on change (for switcher)
  React.useEffect(() => {
    document.documentElement.lang = i18n.resolvedLanguage || 'en';
  }, [i18n.resolvedLanguage]);

  React.useEffect(() => {
    // Update title
    document.title = siteConfig.title;

    // Update favicon
    const link = (document.querySelector("link[rel*='icon']") || document.createElement('link')) as HTMLLinkElement;
    link.type = 'image/png';
    link.rel = 'icon';
    link.href = faviconUrl;
    document.getElementsByTagName('head')[0].appendChild(link);
  }, [faviconUrl, siteConfig.title]);

  return (
    <Router basename={siteConfig.basePath}>
      <AppReadyProvider>
        <LanguagePathSync>
          <Layout>
            <Suspense fallback={null}>
              <Routes>
                <Route path="/lang/:language" element={<Home />} />
                <Route path="/page/:pageNum/lang/:language" element={<Home />} />
                <Route path="/post/:slug/lang/:language" element={<PostDetail />} />
                <Route path="/" element={<Home />} />
                <Route path="/page/:pageNum" element={<Home />} />
                <Route path="/post/:slug" element={<PostDetail />} />
                <Route path="/categories/:category" element={<CategoryDetail />} />
                <Route path="/tags/:tag" element={<TagDetail />} />
                <Route path="/archives" element={<Archives />} />
                <Route path="/archives/:year" element={<Archives />} />
                <Route path="/archives/:year/:month" element={<Archives />} />
                <Route path="/albums" element={albumConfig.enabled ? <Albums /> : <Navigate to="/" replace />} />
                <Route path="/albums/:dirname" element={albumConfig.enabled ? <AlbumDetail /> : <Navigate to="/" replace />} />
                <Route path="/memo" element={memoConfig.enabled ? <Memo /> : <Navigate to="/" replace />} />
              </Routes>
            </Suspense>
          </Layout>
        </LanguagePathSync>
      </AppReadyProvider>
    </Router>
  );
};

export default App;
