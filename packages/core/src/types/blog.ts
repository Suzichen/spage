
export interface LocalizedPostMeta {
  title: string;
  summary: string;
  tags?: string[];
  categories?: string[];
}

export interface PostMetadata {
  slug: string;
  title: string;
  date: string;
  tags: string[];
  categories: string[];
  summary: string;
  prev?: string; // slug of previous post
  next?: string; // slug of next post
  availableLanguages: string[];
  localizedMeta?: Record<string, LocalizedPostMeta>;
}

export type PostManifest = PostMetadata[];
