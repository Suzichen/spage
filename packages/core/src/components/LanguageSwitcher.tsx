import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

const languages: Record<string, string> = {
  'zh-CN': '简体中文',
  'en': 'English',
  'ja': '日本語',
};

const LanguageSwitcher: React.FC = () => {
  const { i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  const isActive = (code: string) =>
    code === 'en' ? i18n.resolvedLanguage?.startsWith('en') : i18n.resolvedLanguage === code;

  const switchLanguage = (language: string) => {
    try {
      window.localStorage.setItem('i18nextLng', language);
    } catch {}
    setOpen(false);
    void i18n.changeLanguage(language);
  };

  return (
    <nav className="relative" ref={ref} aria-label="Language">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center text-secondary hover:text-primary transition-colors focus:outline-none"
        aria-label="Switch language"
        aria-expanded={open}
        aria-haspopup="menu"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M12.65 15.67c.14-.36.05-.77-.23-1.05l-2.09-2.06.03-.03A17.52 17.52 0 0 0 14.07 6h1.94c.54 0 .99-.45.99-.99v-.02c0-.54-.45-.99-.99-.99H10V3c0-.55-.45-1-1-1s-1 .45-1 1v1H1.99c-.54 0-.99.45-.99.99 0 .55.45.99.99.99h10.18A15.66 15.66 0 0 1 9 11.35c-.81-.89-1.49-1.86-2.06-2.88A.885.885 0 0 0 6.16 8c-.69 0-1.13.75-.79 1.35.63 1.13 1.4 2.21 2.3 3.21L3.3 16.87a.99.99 0 0 0 0 1.42c.39.39 1.02.39 1.42 0L9 14l2.02 2.02c.51.51 1.38.32 1.63-.35zM17.5 10c-.6 0-1.14.37-1.35.94l-3.67 9.8c-.24.61.22 1.26.87 1.26.39 0 .74-.24.88-.61l.89-2.39h4.75l.9 2.39c.14.36.49.61.88.61.65 0 1.11-.65.88-1.26l-3.67-9.8c-.22-.57-.76-.94-1.36-.94zm-1.62 7 1.62-4.33L19.12 17h-3.24z"></path></svg>
      </button>
      {open && (
        <ul role="menu" className="absolute right-0 top-full mt-2 bg-bg border border-border rounded shadow-lg py-1 z-50 min-w-[120px] list-none m-0 p-1">
          {Object.entries(languages).map(([code, label]) => (
            <li key={code} role="none">
              <button
                role="menuitem"
                onClick={() => switchLanguage(code)}
                className={`block w-full text-left px-4 py-2 text-sm rounded transition-colors hover:bg-bg-secondary-hover hover:text-primary ${
                  isActive(code) ? 'bg-bg-secondary text-primary font-medium' : 'text-secondary'
                }`}
                aria-current={isActive(code) ? 'true' : undefined}
              >
                {label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </nav>
  );
};

export default LanguageSwitcher;
