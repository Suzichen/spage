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
| `crates/spage-scaffold/src/lib.rs` | `spage.requires`、`spage.core` 和 `@s-page/engine` 的版本字符串 |

> ⚠️ **重点**：`npm/` 下三个平台包的 `version` 必须与主包 `optionalDependencies` 中声明的版本一致。
>
> ℹ️ `lib.rs` 中 `generate_package_json` 的 `spage.core` 必须是精确版本，`@s-page/engine` 固定为当前 Node 入口版本，`spage.requires` 则声明兼容的 engine 能力范围。
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

## 快速操作命令

版本号修改已通过 `scripts/bump-*.js` 自动化。

### 脚本一览

| 命令 | 作用 | 自动修改的文件 |
|------|------|---------------|
| `bun run bump:engine <ver>` | 升级 engine 版本 | Cargo.toml、主包及 3 个平台包 package.json、package-lock.json |
| `bun run bump:core <ver>` | 升级 core 版本 | `packages/core/package.json` |
| `bun run bump:create <ver>` | 升级 create-spage 版本 | Cargo.toml、主包及 3 个平台包 package.json、`lib.rs` 中的依赖版本 |
| `bun run bump:repo` | 升级根项目 engine 依赖 | 根 `package.json` + `bun.lock` |

### 附加参数

**`bump:engine`** 和 **`bump:create`** 支持 `--tag`，加上后会自动 `git add → commit → tag → push`，触发 CI 发布：

```bash
bun run bump:engine 0.6.6 --tag     # commit + 打 engine-v0.6.6 tag + push
bun run bump:create 0.6.4 --tag     # commit + 打 create-v0.6.4 tag + push
```

**`bump:create`** 还支持 `--core` 和 `--engine` 覆盖 scaffold 模板中的依赖版本（不传则自动从对应 package.json 读取当前值）：

```bash
bun run bump:create 0.6.4 --core=0.7.0 --engine=0.6.6 --tag
```

**`bump:repo`** 支持 `--engine` 覆盖版本（不传则自动检测）：

```bash
bun run bump:repo               # 自动读取 engine 当前版本
bun run bump:repo --engine=0.6.6
```

### 典型发版流程

```bash
# 1. bump engine 版本并触发 CI
bun run bump:engine 0.6.6 --tag
# ⏳ 等待 CI 构建 + 发布到 npm

# 2. bump core 版本
bun run bump:core 0.7.0
# (按 publish-core.yml 的触发条件发布)

# 3. bump create-spage 版本并触发 CI（自动拉取 core/engine 最新版本写入 scaffold）
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
4. **scaffold 硬编码版本** — `crates/spage-scaffold/src/lib.rs` 中 `spage.requires`、`spage.core` 和 `@s-page/engine` 版本字符串需要跟着改
5. **发布顺序错误** — 平台包必须先于主包可用，deploy 必须在 engine 发布成功之后
6. **create-spage 正式发布需要 optionalDependencies** — beta 测试时可以直接带 `*.node`，正式发布需要恢复 optionalDependencies 并去掉 `files` 中的 `*.node`
