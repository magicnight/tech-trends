# 部署指南

## 本地开发

### 系统要求

- Rust 1.75+（推荐 stable 最新版）
- Podman（运行 Qdrant）
- Windows：需要 Visual Studio Build Tools + Windows 11 SDK

### 1. 克隆并编译

```bash
git clone https://github.com/yourname/tech-trends.git
cd tech-trends
cargo build --release
```

编译产物位于 `target/release/tech-trends`（单个二进制文件）。

#### Windows 编译注意事项

Git Bash 的 `link.exe`（GNU coreutils）会遮盖 MSVC 链接器，导致编译失败。解决方案：

**方式 A**：使用项目自带的 `build.bat`

```cmd
build.bat
```

该脚本会自动通过 `vcvarsall.bat` 初始化 MSVC 环境。

**方式 B**：在 VS Developer Command Prompt 中编译

```cmd
# 开始菜单 → Developer Command Prompt for VS 2026
cd E:\soft\tech-trands
cargo build --release
```

**方式 C**：使用 PowerShell（通常不受 link.exe 冲突影响）

```powershell
cd E:\soft\tech-trands
cargo build --release
```

### 2. 启动依赖服务

```bash
# Qdrant 向量数据库
podman run -d \
  --name qdrant \
  -p 6334:6334 -p 6333:6333 \
  -v qdrant_data:/qdrant/storage \
  qdrant/qdrant

# Ollama + Embedding 模型
ollama pull nomic-embed-text
```

### 3. 配置环境变量

```bash
# 最小配置
export TECT_LLM_API_KEY="sk-your-deepseek-key"

# 可选：指定数据库路径
export TECT_DB_PATH="$HOME/.local/share/tech-trends/tech-trends.db"
mkdir -p "$HOME/.local/share/tech-trends"
```

### 4. 首次运行

```bash
# 同步数据
tech-trends sync all

# 生成简报
tech-trends digest

# 趋势预测
tech-trends forecast "rust"
```

---

## Podman Compose 一键部署

创建 `podman-compose.yml`：

```yaml
services:
  qdrant:
    image: qdrant/qdrant
    ports:
      - "6334:6334"
      - "6333:6333"
    volumes:
      - qdrant_data:/qdrant/storage
    restart: unless-stopped

  ollama:
    image: ollama/ollama
    ports:
      - "11434:11434"
    volumes:
      - ollama_data:/root/.ollama
    restart: unless-stopped

volumes:
  qdrant_data:
  ollama_data:
```

```bash
# 启动基础设施
podman compose up -d

# 拉取 Embedding 模型
podman exec ollama ollama pull nomic-embed-text

# 运行 tech-trends
tech-trends sync all
```

---

## 生产部署建议

### 定时同步

使用 cron 定期同步数据：

```cron
# 每 6 小时同步全部数据源
0 */6 * * * cd /path/to/tech-trends && ./target/release/tech-trends sync all >> /var/log/tech-trends-sync.log 2>&1

# 每天早上 7 点生成简报
0 7 * * * cd /path/to/tech-trends && ./target/release/tech-trends digest > /path/to/reports/$(date +\%Y-\%m-\%d).md

# 每天凌晨运行全部话题分析
0 1 * * * cd /path/to/tech-trends && ./target/release/tech-trends topic run >> /var/log/tech-trends-topic.log 2>&1
```

### 数据备份

SQLite 是单文件数据库，备份简单：

```bash
# 直接复制（确保没有写入操作时）
cp tech-trends.db tech-trends.db.backup

# 或使用 SQLite 的 .backup 命令（在线备份）
sqlite3 tech-trends.db ".backup 'tech-trends-$(date +%Y%m%d).db'"
```

Qdrant 数据可从 SQLite 重建，不需要独立备份：

```bash
# 如果 Qdrant 数据丢失，重新向量化即可（未来版本将提供 reindex 命令）
```

### 资源需求

| 组件 | 内存 | 磁盘 | 说明 |
|------|------|------|------|
| tech-trends | ~50MB | ~10MB | 单二进制 + SQLite |
| Qdrant | ~200MB | 按数据量 | 向量存储 |
| Ollama | ~2GB | ~300MB | nomic-embed-text 模型 |

### 日志管理

```bash
# 启用详细日志
RUST_LOG=info tech-trends sync all

# 只看 tech-trends 的日志
RUST_LOG=tech_trends=debug tech-trends forecast "rust"

# 生产环境推荐
RUST_LOG=tech_trends=info,warn
```

---

## 全离线运行

tech-trends 支持完全离线使用（同步除外）：

1. **Embedding**：Ollama 本地推理，不联网
2. **LLM**：替换为本地 Ollama 模型

```bash
export TECT_LLM_API_URL=http://localhost:11434/v1
export TECT_LLM_MODEL=llama3
export TECT_LLM_API_KEY=ollama
```

3. **数据库**：SQLite 本地文件
4. **向量库**：Qdrant 本地运行

离线模式下，`digest`、`forecast`、`backtest`、`chat`、`topic run` 全部可用，只有 `sync` 需要网络。

---

## 故障排查

### 编译失败：link.exe 冲突（Windows）

```
error: linking with `link.exe` failed
link: extra operand '...'
```

**原因**：Git Bash 的 `link.exe` 遮盖了 MSVC 链接器。
**解决**：使用 `build.bat` 或 VS Developer Command Prompt。

### 编译失败：找不到 kernel32.lib

```
LINK : fatal error LNK1181: 无法打开输入文件"kernel32.lib"
```

**原因**：Windows SDK 未安装。
**解决**：通过 Visual Studio Installer 安装 Windows 11 SDK，或 `winget install Microsoft.WindowsSDK.10.0.22621`。

### Qdrant 连接失败

```
Failed to create Qdrant collection
```

**原因**：Qdrant 服务未启动。
**解决**：`podman run -p 6334:6334 qdrant/qdrant`

### Ollama Embedding 失败

```
Failed to get embedding from Ollama
```

**原因**：Ollama 服务未启动或模型未下载。
**解决**：
```bash
ollama serve &       # 启动服务
ollama pull nomic-embed-text  # 下载模型
```

### LLM 请求失败

```
Failed to parse LLM response
```

**原因**：API Key 未配置或无效。
**解决**：检查 `TECT_LLM_API_KEY` 环境变量。
