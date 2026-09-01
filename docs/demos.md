# Demo GIF 录制

README 顶部的演示 gif（`docs/screenshots/*.gif`）由脚本化的 REPL 会话
录制，管线沿用 asciinema + agg：

```
scripts/demos/*.demo   →  scripts/record_demo.py（pty 驱动，逐字符打字）
                       →  asciinema rec（生成 .cast）
                       →  agg（渲染成 gif）
```

## 依赖

- `asciinema`（3.x，系统包）
- `agg`（[github.com/asciinema/agg](https://github.com/asciinema/agg)
  releases 下载预编译二进制；注意 release 资产是**裸二进制**不是 tar 包）
- `python3`（标准库即可，无第三方依赖）
- 已构建的 `target/release/scheme-rs`

## 用法

```bash
cargo build --release
# agg 不在 PATH 时用 AGG_BIN 指定；agg 默认字体栈（JetBrains Mono 等）
# 在本机不存在，需用 AGG_FONT 指定一个已安装的等宽字体
AGG_BIN=/tmp/agg-bin/agg AGG_FONT="Source Code Pro" bash scripts/record_demos.sh
```

可覆盖的环境变量：`DEMO_SIZE`（默认 100x26）、`AGG_THEME`（monokai）、
`AGG_SPEED`（1.3）、`AGG_FONT_SIZE`（默认 20，调大可提高清晰度）、
`AGG_BIN`、`AGG_FONT`。

## 新增 / 修改 demo

1. 在 `scripts/demos/` 加一个 `NAME.demo`（或改现有的），格式一行一条：
   - `wait <秒>` 停顿
   - `type <文本>` 逐字符输入（0.07s/字符，模拟人工打字）
   - `key <enter|esc|tab|backspace|space|up|down|left|right|home|end>`
   - `ctrl <字母>`
   - `#` 开头是注释
   - 末尾用 `type (exit)` + `key enter` 退出 REPL
2. 把 NAME 加进 `scripts/record_demos.sh` 的 `ORDER`。
3. 重录后**目检 gif**（内容是否正确、有没有录进意外输出），再更新
   README 的引用。

## 注意事项

- REPL banner 带版本号（`scheme-rs 0.2.1 — ...`），版本升级后 gif 里的
  版本号会过时，按需重录。
- 录制从仓库根目录启动 REPL，`require` 走 `src/libs/` 搜索路径，直接可用。
- `.cast` 中间产物在 `/tmp/scheme-rs-demo/out/`，不进仓库。
- 脚本带看门狗：超时自动发 `(exit)` 再 SIGTERM，不会挂死录制会话。
