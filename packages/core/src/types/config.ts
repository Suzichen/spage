export interface SocialLinkItem {
  platform: string;
  url?: string;
  icon?: string;
  label?: string;
}

export interface LinksConfig {
  enabled: boolean;
  items: Record<string, string>;
}

export interface SocialLinksConfig {
  enabled: boolean;
  items: SocialLinkItem[];
}

export interface SiteConfig {
  title: string;
  description: string;
  logo: string;
  favicon: string;
  siteUrl?: string;
  author?: string;
  language?: string;
  timezone?: string;
  basePath?: string;
  links?: LinksConfig;
  socialLinks?: SocialLinksConfig;
}
