import React, { useEffect, useState, useRef, useCallback, useMemo } from 'react';
import { useParams, Link, useLocation, useNavigationType, useNavigate } from 'react-router-dom';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeSlug from 'rehype-slug';
import TableOfContents from '@/components/TableOfContents';
import StickyToc from '@/components/StickyToc';
import Prism from 'prismjs';
import 'prismjs/themes/prism.css';
import { useTranslation } from 'react-i18next';
import { usePost } from '@/hooks/usePost';
import { usePosts } from '@/hooks/usePosts';
import { restoreScrollForKey } from '@/hooks/useScrollToTop';
import ImageWithCaption from '@/components/ImageWithCaption';
import PhotoViewer from '@/components/PhotoViewer';
import { ArticleSkeleton } from '@/components/Skeleton';
import { useSignalReady } from '@/AppReadyProvider';
import type { PhotoItem } from '@/types/album';

const PostDetail: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { slug } = useParams<{ slug: string }>();
  const { hash, key } = useLocation();
  const navType = useNavigationType();
  const navigate = useNavigate();

  const { post, content, loading, isFallback, prevPost, nextPost } = usePost(slug);
  const { posts } = usePosts();

  // Localized date formatting
  const formattedDate = useMemo(() => {
    if (!post) return '';
    const date = new Date(post.date);
    const lang = i18n.resolvedLanguage ?? 'en';
    if (lang === 'zh-CN' || lang.startsWith('zh')) {
      return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`;
    } else if (lang === 'ja') {
      return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`;
    } else {
      return date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: '2-digit' });
    }
  }, [post, i18n.resolvedLanguage]);

  // Related posts: same category OR same tag, sorted by date desc, max 5
  const relatedPosts = useMemo(() => {
    if (!post || posts.length === 0) return [];
    const currentCategories = new Set(post.categories);
    const currentTags = new Set(post.tags);

    return posts
      .filter((p) => {
        if (p.slug === post.slug) return false;
        const hasCommonCategory = p.categories.some((c) => currentCategories.has(c));
        const hasCommonTag = p.tags.some((t) => currentTags.has(t));
        return hasCommonCategory || hasCommonTag;
      })
      .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())
      .slice(0, 5);
  }, [post, posts]);

  const [viewerOpen, setViewerOpen] = useState(false);
  const [viewerIndex, setViewerIndex] = useState(0);
  const [viewerPhotos, setViewerPhotos] = useState<PhotoItem[]>([]);
  const imagesRef = useRef<{ src: string; caption: string }[]>([]);
  const mobileTocRef = useRef<HTMLDivElement>(null);
  const [stickyToc, setStickyToc] = useState(false);

  // Reset sticky state on page navigation
  useEffect(() => { setStickyToc(false); }, [slug]);

  // Reset every render - ReactMarkdown will re-register all images
  imagesRef.current = [];

  const openViewer = useCallback((index: number) => {
    setViewerPhotos(imagesRef.current.map((img) => ({
      filename: img.caption,
      thumbnailUrl: img.src,
      originalUrl: img.src,
      exif: { cameraMake: null, cameraModel: null, focalLength: null, aperture: null, shutterSpeed: null, iso: null },
    })));
    setViewerIndex(index);
    setViewerOpen(true);
  }, []);

  // After content renders: handle hash scroll or POP restore
  useEffect(() => {
    if (!content) return;
    Prism.highlightAll();

    if (navType === 'POP') {
      const restored = restoreScrollForKey(key);
      if (!restored && hash) {
        const id = decodeURIComponent(hash.slice(1));
        requestAnimationFrame(() => {
          document.getElementById(id)?.scrollIntoView();
        });
      }
    } else if (hash) {
      const id = decodeURIComponent(hash.slice(1));
      requestAnimationFrame(() => {
        document.getElementById(id)?.scrollIntoView();
      });
    }
  }, [content]);

  // Handle hash changes within the same page (TOC clicks)
  useEffect(() => {
    if (!content || !hash) return;
    const id = decodeURIComponent(hash.slice(1));
    const el = document.getElementById(id);
    if (el) el.scrollIntoView();
  }, [hash]);

  // Mobile sticky TOC: observe when inline TOC leaves viewport
  useEffect(() => {
    const el = mobileTocRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting && entry.boundingClientRect.top < 0) {
        setStickyToc(true);
      } else {
        setStickyToc(false);
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [content]);

  useSignalReady(!loading);

  if (loading) {
    return <ArticleSkeleton />;
  }

  if (!post) {
    return <div>{t('common.postNotFound')}</div>;
  }

  return (
    <div className="relative w-full max-w-[800px] mx-auto pb-8 px-2 md:px-4 xl:px-0 xl:max-w-content">
      <article>
        {isFallback && (
          <div className="mb-4 px-4 py-2 bg-amber-50 dark:bg-amber-900/20 text-amber-800 dark:text-amber-200 text-sm rounded-lg">
            {t('post.noLocalizedVersion')}
          </div>
        )}
        <header className="mb-8 border-b-0">
          <h1 className="text-4xl md:text-5xl font-normal mb-2 leading-tight text-center">{post.title}</h1>
          <div className="flex items-baseline text-sm text-secondary mx-4 md:mx-[120px]">
            <span className="shrink-0 mr-4">{formattedDate}</span>
            {post.tags.length > 0 && (
              <div className="flex flex-wrap gap-y-1 ml-auto">
                {post.tags.map((tag) => (
                  <Link
                    key={tag}
                    to={`/tags/${tag}`}
                    className="text-sm text-secondary hover:text-accent bg-bg-secondary px-1 py-0.5 rounded hover:bg-bg-secondary-hover transition-colors no-underline"
                  >
                    #{tag}
                  </Link>
                ))}
              </div>
            )}
          </div>
        </header>

        {/* Mobile TOC - inline collapsible */}
        <div ref={mobileTocRef} className="xl:hidden mb-6">
          <TableOfContents content={content} collapsible />
        </div>

        {/* Mobile TOC - sticky when scrolled past */}
        <StickyToc content={content} visible={stickyToc} />

        <div className="relative">
          <div className="markdown-body">
            <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeSlug]}
                components={{
                  img: ({ src, alt, title }) => {
                    const caption = title || alt || '';
                    const index = imagesRef.current.length;
                    if (src) {
                      imagesRef.current.push({ src, caption });
                    }
                    return (
                      <ImageWithCaption
                        src={src}
                        alt={alt}
                        title={title}
                        onClick={() => src && openViewer(index)}
                      />
                    );
                  },
                  a: ({ node, href, ...props }) => {
                    if (href?.startsWith('#')) {
                      return (
                        <a
                          href={href}
                          onClick={(e) => {
                            e.preventDefault();
                            // Only navigate if it's an internal hash link
                            navigate(`${window.location.pathname}${href}`);
                          }}
                          {...props}
                        />
                      );
                    }
                    return <a href={href} {...props} />;
                  }
                }}
            >
                {content}
            </ReactMarkdown>
          </div>

          {/* Desktop Related Posts Sidebar - aligned with markdown body */}
          {relatedPosts.length > 0 && (
            <aside className="hidden xl:block absolute top-0 -left-[300px] 2xl:-left-[360px] h-full w-[260px]">
              <div className="sticky top-[120px]">
                <section>
                  <h3 className="text-sm uppercase tracking-wider font-bold text-secondary mb-4 pb-2 border-b border-border">
                    {t('post.relatedPosts')}
                  </h3>
                  <ul className="list-none p-0 m-0">
                    {relatedPosts.map((relPost) => (
                      <li key={relPost.slug} className="mb-3">
                        <Link
                          to={`/post/${relPost.slug}`}
                          className="block text-sm text-primary hover:text-accent no-underline transition-colors leading-snug"
                        >
                          {relPost.title}
                        </Link>
                      </li>
                    ))}
                  </ul>
                </section>
              </div>
            </aside>
          )}
        </div>

        <hr className="my-12 border-border" />

        <nav className="flex justify-between flex-wrap gap-4">
          <div className="flex-1 min-w-[200px]">
            {prevPost && (
              <Link to={`/post/${prevPost.slug}`} className="block group no-underline">
                <div className="text-sm text-secondary mb-1">{t('common.prevPost')}</div>
                <div className="text-lg font-bold group-hover:text-accent transition-colors">
                  &laquo; {prevPost.title}
                </div>
              </Link>
            )}
          </div>
          <div className="flex-1 min-w-[200px] text-right">
            {nextPost && (
              <Link to={`/post/${nextPost.slug}`} className="block group no-underline">
                <div className="text-sm text-secondary mb-1">{t('common.nextPost')}</div>
                <div className="text-lg font-bold group-hover:text-accent transition-colors">
                  {nextPost.title} &raquo;
                </div>
              </Link>
            )}
          </div>
        </nav>
      </article>

      {/* Desktop TOC Sidebar */}
      <aside className="hidden xl:block absolute top-0 -right-[340px] 2xl:-right-[360px] h-full w-[300px]">
        {/* Sticky Inner */}
        <div className="sticky top-[120px] max-h-[calc(100vh-140px)] overflow-y-auto pr-2 custom-scrollbar">
            <TableOfContents content={content} />
        </div>
      </aside>

      {viewerOpen && (
        <PhotoViewer
          photos={viewerPhotos}
          initialIndex={viewerIndex}
          onClose={() => setViewerOpen(false)}
        />
      )}
    </div>
  );
};

export default PostDetail;
