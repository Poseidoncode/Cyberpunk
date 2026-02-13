# Cyberpunk Project Management Makefile

.PHONY: help setup dev build dmg clean check format lint

# 預設目標：顯示說明
help:
	@echo "Cyberpunk - Enterprise-Grade Git Terminal"
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  setup    - Install all dependencies (npm & cargo)"
	@echo "  dev      - Start the Tauri development environment"
	@echo "  build    - Build the production application (.app)"
	@echo "  dmg      - Build and package as DMG installer"
	@echo "  check    - Run type checking and rust diagnostics"
	@echo "  format   - Automatically format all source code"
	@echo "  clean    - Remove build artifacts and temporary files"

# 初始化開發環境
setup:
	@echo "📦 Installing frontend dependencies..."
	npm install
	@echo "🦀 Checking backend dependencies..."
	cd src-tauri && cargo fetch

# 啟動開發伺服器
dev:
	@echo "🚀 Starting Cyberpunk in dev mode..."
	npm run tauri dev
# 打包生產版本 (.app)
build:
	@echo "🏗️ Building production bundle..."
	npm run tauri build -- --ci

# 打包 DMG 安裝映像
APP_NAME := Cyberpunk
APP_PATH := src-tauri/target/release/bundle/macos/$(APP_NAME).app
DMG_PATH := src-tauri/target/release/bundle/dmg/$(APP_NAME).dmg

dmg: build
	@echo "📦 Creating DMG installer..."
	@mkdir -p src-tauri/target/release/bundle/dmg
	@rm -f "$(DMG_PATH)"
	@STAGING=$$(mktemp -d) && \
		cp -R "$(APP_PATH)" "$$STAGING/" && \
		ln -s /Applications "$$STAGING/Applications" && \
		hdiutil create -volname "$(APP_NAME)" -srcfolder "$$STAGING" \
			-ov -format UDZO "$(DMG_PATH)" && \
		rm -rf "$$STAGING"
	@echo "✅ DMG created: $(DMG_PATH)"


# 靜態分析與檢查
check:
	@echo "🔍 Checking TypeScript..."
	npm run build
	@echo "🔍 Checking Rust..."
	cd src-tauri && cargo check

# 程式碼格式化
format:
	@echo "🎨 Formatting frontend code..."
	npx prettier --write "src/**/*.{vue,ts,css}"
	@echo "🎨 Formatting backend code..."
	cd src-tauri && cargo fmt

# 清理環境
clean:
	@echo "🧹 Cleaning dist and target folders..."
	rm -rf dist
	rm -rf src-tauri/target
	@echo "✨ Clean completed."
