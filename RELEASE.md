# spage 发版指南

## 包依赖关系

```
create-spage (CLI脚手架, 含 NAPI native binding)
  ├─ optionalDependencies →
  │    create-spage-darwin-arm64
  │    create-spage-linux-x64-gnu
  │    create-spage-win32-x64-msvc
  └─ 生成的项目声明 → spage.core + devDependency @s-page/engine

@s-page/engine (主包)
  └─ optionalDependencies →
       @s-page/engine-darwin-arm64
       @s-page/engine-linux-x64-gnu
       @s-page/engine-win32-x64-msvc

根项目 (monorepo开发用)
  ├─ @s-page/core (workspace link)
  └─ @s-page/engine (固定版本)
```

## 版本号修改 Checklist

### 1. Engine 相关（7处）

| 文件 | 字段 |
|------|------|
| `crates/spage-engine/Cargo.toml` | `version`（Rust crate 版本，需与 `@s-page/engine` 保持一致） |
| `crates/spage-engine-napi/package.json` | `version` |
| `crates/spage-engine-napi/package.json` | `optionalDependencies` 下三个平台包版本 |
| `crates/spage-engine-napi/npm/darwin-arm64/package.json` | `version` |
| `crates/spage-engine-napi/npm/linux-x64-gnu/package.json` | `version` |
| `crates/spage-engine-napi/npm/win32-x64-msvc/package.json` | `version` |
| `crates/spage-engine-napi/package-lock.json` | `version` + `optionalDependencies` |

> ⚠️ **重点**：`npm/` 下三个平台包的 `version` 必须与主包 `optionalDependencies` 中声明的版本一致，否则 CI 发布后用户安装时版本不匹配，运行时会报 `Cannot find module '@s-page/engine-<platform>'`。
>
> ℹ️ `crates/spage-engine/Cargo.toml` 的 `version` 让直接以 Rust crate 形式引用 engine 的仓库能正确获知版本号，应与 `@s-page/engine` 的 npm 版本保持同步。改完后运行 `cargo update -p spage-engine` 同步 `Cargo.lock`。

### 2. Core（1处）

| 文件 | 字段 |
|------|------|
| `packages/core/package.json` | `version` |

### 3. Create-spage（6处）

| 文件 | 字段 |
|------|------|
| `crates/spage-scaffold/Cargo.toml` | `version`（Rust crate 版本，需与 `create-spage` 保持一致） |
| `packages/create-spage/package.json` | `version` |
| `packages/create-spage/package.json` | `optionalDependencies` 下三个平台包版本 |
| `packages/create-spage/npm/darwin-arm64/package.json` | `version` |
| `packages/create-spage/npm/linux-x64-gnu/package.json` | `version` |
| `packages/create-spage/npm/win32-x64-msvc/package.json` | `version` |
| `crates/spage-scaffold/src/lib.rs` | `spage.core` 和 `@s-page/engine` 的版本字符串 |

> ⚠️ **重点**：`npm/` 下三个平台包的 `version` 必须与主包 `optionalDependencies` 中声明的版本一致。
>
> ℹ️ `lib.rs` 中 `generate_package_json` 的 `spage.core` 必须是精确版本，`@s-page/engine` 固定为当前 Node 入口版本；两者的发布线约束见「core ⇄ engine 版本约束」。
>
> ℹ️ `crates/spage-scaffold/Cargo.toml` 的 `version` 让直接以 Rust crate 形式引用 scaffold 的仓库能正确获知版本号，应与 `create-spage` 的 npm 版本保持同步。改完后运行 `cargo update -p spage-scaffold` 同步 `Cargo.lock`。

### 4. 根项目依赖（1处）

| 文件 | 字段 |
|------|------|
| `package.json` | `dependencies["@s-page/engine"]` |

> ⚠️ 改完版本号后运行 `bun install` 更新 `bun.lock`（需 engine 已发布到 npm）。

### 5. Schema（按需）

如果 `config.json` 或 `album.config.json` 的结构有变化：
- `packages/core/schemas/config.schema.json`
- `packages/core/schemas/album.config.schema.json`

> ℹ️ create-spage 在编译时内嵌这些 schema 用于初始化新项目，改完需重新发布 create-spage 才会生效；已有项目在下一次 serve/build 时自动获得新 core 的 schema。

## CI 发布顺序

```
1. git tag engine-v{VERSION}  →  触发 build-engine.yml
   ├─ 构建三平台 native .node 文件
   ├─ 发布 @s-page/engine-darwin-arm64
   ├─ 发布 @s-page/engine-linux-x64-gnu
   ├─ 发布 @s-page/engine-win32-x64-msvc
   └─ 发布 @s-page/engine (主包)

2. 手动触发 publish-core.yml 或按其触发条件
   └─ 发布 @s-page/core

3. git tag create-v{VERSION}  →  触发 build-create-spage.yml
   ├─ 构建三平台 native .node 文件
   ├─ 发布 create-spage-darwin-arm64
   ├─ 发布 create-spage-linux-x64-gnu
   ├─ 发布 create-spage-win32-x64-msvc
   └─ 发布 create-spage (主包)

4. push master  →  触发 deploy.yml
   └─ bun install → build → deploy to Cloudflare Pages
```

> **关键**：deploy.yml 用 `bun install`，engine 必须已发布到 npm，否则安装会失败。

## core ⇄ engine 版本约束

engine 只接受同一发布线的 core：`0.x` 要求 major+minor 相同，`1.x` 起要求 major 相同。不满足时 serve/build 报 `CoreVersionMismatch`，且 `spage update core` 只在该发布线内选版本。

bump 脚本据此校验，规则不对称：

| 命令 | 目标版本与另一侧不同线时 |
|---|---|
| `bump:core` | warn，继续 |
| `bump:engine` | 中止，要求先 `bump:core` |
| `bump:create` | 中止 |

理由：core 领先安全（老 engine 不会选到新线的 core），engine 领先会让用户既报错又无版本可升。

### 同发布线（常规）

各包独立发布，无先后要求，按上文「CI 发布顺序」执行。

### 跨发布线（0.6 → 0.7）

core 必须先在 npm 上可用，因此不能用 `bump:engine --tag`（它会立即推 tag 触发 engine 发布）：

```bash
bun run bump:core 0.7.0
bun run bump:engine 0.7.0            # 不加 --tag
git add -A && git commit -m "chore: bump core & engine to 0.7.0" && git push
# 手动触发 publish-core.yml，等 @s-page/core@0.7.0 发布完成
git tag engine-v0.7.0 && git push origin engine-v0.7.0
# engine 发布后更新根 package.json 的 @s-page/engine + bun install
bun run bump:create <ver> --tag
```

两个 bump 必须在同一提交：中间状态（core 0.7 / engine 0.6）会让 `cargo test` 报 `CoreVersionMismatch`。这个提交推送 master 触发的 deploy 不受影响，它用已发布的 engine 加本地构建的 core。

### 预发布

版本号必须带 prerelease 后缀，如 `0.6.11-beta.1`：

- `spage update` 按 semver 选发布线内最高的**非 prerelease** 版本，纯数字版本号会被当成稳定版发给所有用户；
- publish workflow 由版本号推导 dist-tag（`0.6.11-beta.1` → `beta`，无后缀 → `latest`），带后缀才不会顶掉 `latest`。

## 快速操作命令

版本号修改已通过 `scripts/bump-*.js` 自动化。

### 脚本一览

| 命令 | 作用 | 自动修改的文件 |
|------|------|---------------|
| `bun run bump:engine <ver>` | 升级 engine 版本 | Cargo.toml、主包及 3 个平台包 package.json、package-lock.json |
| `bun run bump:core <ver>` | 升级 core 版本 | `packages/core/package.json` |
| `bun run bump:create <ver>` | 升级 create-spage 版本 | Cargo.toml、主包及 3 个平台包 package.json、`lib.rs` 中的依赖版本 |
| `bun run bump:repo` | 升级根项目 engine 依赖 | 根 `package.json` + `bun.lock` |

`bump:create` 写入 scaffold 的 core / engine 引脚只取自源码树（`packages/core/package.json`、`crates/spage-engine-napi/package.json`），没有覆盖参数：scaffold 同时在编译期内嵌 `packages/core/schemas`，引脚与源码树不一致就会让新项目带上另一个版本的 schema。因此顺序固定为先 `bump:core` / `bump:engine`，再 `bump:create`。

### 附加参数

**`bump:engine`** 和 **`bump:create`** 支持 `--tag`，加上后会自动 `git add → commit → tag → push`，触发 CI 发布：

```bash
bun run bump:engine 0.6.6 --tag     # commit + 打 engine-v0.6.6 tag + push
bun run bump:create 0.6.4 --tag     # commit + 打 create-v0.6.4 tag + push
```

**`bump:repo`** 支持 `--engine` 覆盖版本（不传则自动检测）：

```bash
bun run bump:repo               # 自动读取 engine 当前版本
bun run bump:repo --engine=0.6.6
```

### 典型发版流程

同发布线的常规发版（跨发布线见「core ⇄ engine 版本约束」）：

```bash
# 1. bump core 版本并按 publish-core.yml 的触发条件发布
bun run bump:core 0.6.11

# 2. bump engine 版本并触发 CI
bun run bump:engine 0.6.9 --tag
# ⏳ 等待 CI 构建 + 发布到 npm

# 3. bump create-spage 版本并触发 CI（scaffold 引脚取源码树里的 core/engine 版本）
bun run bump:create 0.6.4 --tag
# ⏳ 等待 CI 构建 + 发布到 npm

# 4. engine 发布成功后，更新根项目依赖
bun run bump:repo

# 5. 部署博客
git add -A && git commit -m "chore: update deps" && git push origin master
```

> ⚠️ `bump:repo` 必须在 engine 发布到 npm **之后**执行，否则 `bun install` 会因找不到新版本而失败。

### 不加 --tag 试跑

不加 `--tag` 时脚本只修改本地文件不推送，可以先跑一遍检查改动：

```bash
bun run bump:engine 0.6.6     # 只改文件
git diff                       # 检查改动
git checkout .                 # 撤回，重新跑带 --tag 的
```

## 常见踩坑

1. **平台包版本未同步** — `npm/*/package.json` 的 `version` 必须改，CI 发布时读的就是这个值（engine 和 create-spage 各有一组）
2. **lock 文件未更新** — engine 发布后需 `bun install` 更新 `bun.lock`
3. **engine 子目录有独立 lock** — `crates/spage-engine-napi/package-lock.json` 也需要更新
4. **scaffold 硬编码版本** — `crates/spage-scaffold/src/lib.rs` 中 `spage.core` 和 `@s-page/engine` 版本字符串需要跟着改，并保持在同一兼容发布线
5. **发布顺序错误** — 平台包必须先于主包可用，deploy 必须在 engine 发布成功之后；跨发布线时 core 必须先手动发布，engine tag 要等 core 上线后再推（见「core ⇄ engine 版本约束」）
6. **create-spage 正式发布需要 optionalDependencies** — beta 测试时可以直接带 `*.node`，正式发布需要恢复 optionalDependencies 并去掉 `files` 中的 `*.node`
7. **beta 版本号没带后缀** — 预发布必须是 `0.6.11-beta.1` 这种形式，纯数字版本会被 `spage update` 当成稳定版，也会顶掉 npm 的 `latest`
