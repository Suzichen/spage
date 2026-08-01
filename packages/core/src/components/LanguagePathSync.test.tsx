import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router-dom';
import LanguagePathSync from './LanguagePathSync';

const i18n = { resolvedLanguage: 'en', changeLanguage: vi.fn() };

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n }),
}));

vi.mock('../context', () => ({
  useSiteConfig: () => ({ language: 'en' }),
}));

vi.mock('../hooks/usePosts', () => ({
  usePosts: () => ({
    posts: [{ slug: 'hello', availableLanguages: ['ja'] }],
    loading: false,
    error: null,
  }),
}));

const CurrentLocation = () => {
  const location = useLocation();
  return <div data-testid="location">{`${location.pathname}${location.search}${location.hash}`}</div>;
};

function renderSync(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <LanguagePathSync>
        <CurrentLocation />
      </LanguagePathSync>
    </MemoryRouter>,
  );
}

describe('LanguagePathSync', () => {
  beforeEach(() => {
    i18n.resolvedLanguage = 'en';
    i18n.changeLanguage.mockReset();
    i18n.changeLanguage.mockImplementation(async (language: string) => {
      i18n.resolvedLanguage = language;
    });
    window.localStorage.clear();
  });

  it('persists and applies an explicit language entry', async () => {
    renderSync('/post/hello/lang/ja');

    await waitFor(() => {
      expect(i18n.changeLanguage).toHaveBeenCalledWith('ja');
    });
    expect(window.localStorage.getItem('i18nextLng')).toBe('ja');
  });

  it('lets a manual language switch replace an already-consumed entry', async () => {
    const view = renderSync('/post/hello/lang/ja');

    await waitFor(() => {
      expect(i18n.changeLanguage).toHaveBeenCalledWith('ja');
    });
    i18n.resolvedLanguage = 'en';
    view.rerender(
      <MemoryRouter initialEntries={['/post/hello/lang/ja']}>
        <LanguagePathSync>
          <CurrentLocation />
        </LanguagePathSync>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId('location').textContent).toBe('/post/hello');
    });
  });

  it('removes invalid language entries', async () => {
    renderSync('/post/hello/lang/fr?source=share#intro');

    await waitFor(() => {
      expect(screen.getByTestId('location').textContent).toBe('/post/hello?source=share#intro');
    });
  });
});
