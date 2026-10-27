.DEFAULT_GOAL := help

CARGO ?= cargo
NPM ?= npm

.PHONY: help build package build-api build-web install api web clean

help:
	@echo 'make build      打包完整应用到 dist/alphak/'
	@echo 'make clean      删除构建产物、依赖与缓存（保留数据库、备份和导入数据）'
	@echo 'scripts/start.sh / scripts/stop.sh  后台启动 / 停止打包应用'
	@echo 'make install    按 lockfile 安装前端依赖'
	@echo 'make api        前台启动后端（先执行 make build）'
	@echo 'make web        前台启动前端开发服务'
	@echo '停止服务：在对应终端按 Ctrl+C'

clean:
	@if [ -f dist/alphak/run/alphak.pid ]; then \
		echo '停止正在运行的 AlphaK…'; \
		./scripts/stop.sh || { echo '停止失败：请先手动执行 ./scripts/stop.sh 再重试。'; exit 1; }; \
	fi
	@echo '删除构建产物、依赖与缓存…'
	rm -rf apps/api/target
	rm -rf apps/web/node_modules apps/web/dist apps/web/test-results apps/web/playwright-report
	rm -rf dist/alphak/bin dist/alphak/web dist/alphak/scripts dist/alphak/logs dist/alphak/run dist/alphak/.env.example
	@echo '已保留：dist/alphak/alphak.db*、dist/alphak/backups/、backups/、data/、.env。'
	@echo '如 make web 仍在运行，请重启前端开发服务。清理后执行 make build 重新构建。'

build: harness package

package: build-api build-web
	mkdir -p dist/alphak/bin dist/alphak/web dist/alphak/scripts
	cp apps/api/target/release/alphak-api dist/alphak/bin/alphak-api
	rm -rf dist/alphak/web
	cp -R apps/web/dist dist/alphak/web
	cp scripts/start.sh scripts/stop.sh scripts/backup.sh scripts/restore.sh scripts/storage-backup.py dist/alphak/scripts/
	cp .env.example dist/alphak/.env.example
	@echo '打包完成：dist/alphak；运行 scripts/start.sh 启动'

build-api:
	$(CARGO) build --release --locked --manifest-path apps/api/Cargo.toml

build-web: install
	$(NPM) --prefix apps/web run build

install:
	$(NPM) --prefix apps/web ci

api:
	./apps/api/target/release/alphak-api

web:
	$(NPM) --prefix apps/web run dev -- --strictPort

.PHONY: harness persistence-doc-update
harness:
	python3 scripts/persistence-harness.py --self-test
	python3 scripts/test-storage-backup.py

persistence-doc-update:
	python3 scripts/persistence-harness.py --update-schema
