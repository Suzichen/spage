export interface AlbumEntry {
  dir: string;       // 相对于 albums/ 的目录名，即 dirname
  name?: string;     // 可选显示名称
  desc?: string;     // 可选相册描述，支持换行
  cover?: string;    // 可选封面文件名
}

export interface AlbumConfig {
  enabled: boolean;
  albums: AlbumEntry[];
}
