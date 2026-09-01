# scheme-rs 开发指引

R5RS Scheme 解释器（Rust）。设计细节见 `docs/architecture.md`（先看这个
再动求值器）。

## 测试

- 统一入口：`bash scripts/test.sh`（fmt --check + clippy -D warnings +
  cargo test，与 CI 一致；`make test` 等价）。改代码后必须全绿再提交。
- `--quick` 跳过最慢的 4 个真实程序用例；`--coverage` 跑 cargo-llvm-cov
  （门禁 70 行覆盖）。
- 测试里往 `target/tmp/` 写临时文件前先 `create_dir_all`（干净环境不存在
  该目录，CI 踩过坑）。

## 提交约定

- 提交信息小写开头、英文、聚焦单一关注点（见 git log）。

## 发版（tag 触发 release）

1. 改 `Cargo.toml` 的 version，**并运行 `cargo check` 同步 `Cargo.lock`**
   （漏掉会被 release 工作流的 `--locked` 拒掉）。
2. `git tag -a vX.Y.Z -m "vX.Y.Z" && git push origin main vX.Y.Z`。
3. `.github/workflows/release.yml` 自动构建 linux/macOS/Windows 三包
   （二进制 + `lib/` 标准库）并创建 GitHub Release。

## 标准库（src/libs/*.scm）

运行时磁盘加载（不内嵌）：搜索路径为可执行文件旁 `lib/` → `./lib/` →
`./src/libs/`。改 .scm 无需重新编译。详见 `docs/extensions.md`。

## 录制 README 演示 gif

见 `docs/demos.md`（asciinema + agg，`bash scripts/record_demos.sh`）。
