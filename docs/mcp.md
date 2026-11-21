# Krono Local MCP Server (`krono-mcp`)

Локальный `stdio` MCP-сервер (JSON-RPC 2.0) поверх `krono-core` без сетевых портов и без доступа к зашифрованным заметкам.

Local `stdio` MCP server (JSON-RPC 2.0) built on `krono-core` with no open network ports and no access to encrypted notes.

## Сборка / Build

```bash
cargo build --release -p krono-mcp
```

Бинарник / Binary:
- Windows: `target/release/krono-mcp.exe`
- macOS / Linux: `target/release/krono-mcp`

## Безопасность / Security Model

- **Fail-closed по умолчанию:** без `--allow-dir <rel_dir>` (или явного `--allow-all`) доступ к файлам закрыт.
- **Read-Only по умолчанию:** инструмент `append` отключён и скрыт из `tools/list`, пока не передан `--allow-write`. В MCP-схеме `append` помечен `readOnlyHint: false`.
- **Запрет на секреты:** файлы `.md.enc`, скрытые папки `.krono/` и `.git/`, а также симлинки за пределы разрешённых директорий полностью невидимы для всех инструментов.
- **Чистый `stdio`:** `stdout` используется строго для JSON-RPC 2.0 сообщений, вся диагностика пишется в `stderr`.

## Инструменты / Tools

| Tool | Args | Mode | Description |
| :--- | :--- | :--- | :--- |
| `list_files` | `folder?` | Read-Only | Компактный список `.md`-файлов внутри разрешённых папок. |
| `search` | `query`, `limit?`, `folder?` | Read-Only | Поиск (`tag:`, `status:`, `path:`, текст) через read-only `.krono/index.db` с fallback на обход `.md`. |
| `read` | `path`, `max_lines?` | Read-Only | Чтение `.md`-файла с защитой от выгрузки гигантских файлов (по умолчанию 400 строк / 64 КБ). |
| `append` | `path`, `content`, `heading?` | `--allow-write` | Атомарная дозапись текста в конец файла или под указанный заголовок `## ...`. |

## Конфигурация клиентов / Client Configs

Ниже — пример ограничения доступа только папкой `notes` в режиме Read-Only. Чтобы разрешить `append`, добавьте `"--allow-write"` в конец массива `"args"`.

Below is an example restricting access to the `notes` folder in Read-Only mode. Add `"--allow-write"` to `"args"` to enable `append`.

### Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "krono": {
      "command": "C:/path/to/krono-mcp.exe",
      "args": [
        "--vault",
        "C:/path/to/krono_vault",
        "--allow-dir",
        "notes"
      ]
    }
  }
}
```

### Cursor (`.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "krono": {
      "command": "C:/path/to/krono-mcp.exe",
      "args": [
        "--vault",
        "C:/path/to/krono_vault",
        "--allow-dir",
        "notes",
        "--allow-write"
      ]
    }
  }
}
```

### Antigravity (`mcp_config.json`)

```json
{
  "mcpServers": {
    "krono": {
      "command": "C:/path/to/krono-mcp.exe",
      "args": [
        "--vault",
        "C:/path/to/krono_vault",
        "--allow-dir",
        "notes"
      ]
    }
  }
}
```
