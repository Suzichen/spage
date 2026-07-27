export interface ExifData {
  cameraMake: string | null;
  cameraModel: string | null;
  focalLength: string | null;
  aperture: string | null;
  shutterSpeed: string | null;
  iso: string | null;
}

export interface PhotoItem {
  filename: string;
  thumbnailUrl: string;   // /albums/{dirname}/thumbs/{filename}.webp
  originalUrl: string;    // /albums/{dirname}/{filename}
  exif: ExifData;
}

export interface AlbumSummary {
  dirname: string;
  name: string;
  cover: string | null;   // 封面缩略图相对 URL，未配置时为 null
  photoCount: number;
}

export interface AlbumDetail {
  dirname: string;
  name: string;
  desc?: string;
  photos: PhotoItem[];
}
