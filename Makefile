# 常用命令的一键入口。完整测试逻辑在 scripts/test.sh（本地与 CI 共用），
# 这里只做薄封装，避免两份逻辑漂移。

.PHONY: help build repl test quick coverage bench fmt clippy clean

help: ## 列出全部指令
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  make %-10s %s\n", $$1, $$2}'

build: ## 编译 debug 版
	cargo build

repl: ## 进入 REPL
	cargo run

test: ## 完整测试门禁：fmt --check + clippy + cargo test（与 CI 一致）
	bash scripts/test.sh

quick: ## 快速冒烟：cargo test，跳过最慢的 4 个真实程序用例
	bash scripts/test.sh --quick

coverage: ## 行覆盖率报告（需要 cargo-llvm-cov，阈值 70）
	bash scripts/test.sh --coverage

bench: ## criterion 基准
	cargo bench --bench interpreter

fmt: ## 自动格式化
	cargo fmt

clippy: ## lint（-D warnings）
	cargo clippy --all-targets -- -D warnings

clean: ## 清理构建产物
	cargo clean
