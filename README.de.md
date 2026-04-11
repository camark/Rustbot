# RustBot

🐈 Ultraleichtes persönliches KI-Assistenten-Framework (Rust-Implementierung)

Dies ist eine komplette Rust-Neuimplementierung von [nanobot](https://github.com/HKUDS/nanobot), entwickelt für:

- **Leistung**: 10-100x schneller als Python
- **Wenig Speicher**: Kein GC, minimaler Fußabdruck
- **Einzelne Binärdatei**: Einfache Bereitstellung, keine Abhängigkeiten
- **Typsicherheit**: Garantien zur Kompilierzeit

## Projektstruktur

```
RustBot/
├── Cargo.toml              # Workspace-Root
├── crates/
│   ├── nanobot-config/     # Konfigurationssystem
│   ├── nanobot-providers/  # LLM-Anbieter
│   ├── nanobot-bus/        # Nachrichten-Bus
│   ├── nanobot-core/       # Agenten-Engine
│   └── nanobot-cli/        # CLI-Schnittstelle
└── docs/
```

## Schnellstart

### Bauen

```bash
cargo build --release
```

### Initialisieren

```bash
./target/release/rustbot onboard
```

### Chatten

```bash
./target/release/rustbot agent -m "Hallo!"
```

### Interaktiver Modus

```bash
./target/release/rustbot agent
```

### Status

```bash
./target/release/rustbot status
```

## Konfiguration

Konfigurationsdatei: `~/.nanobot/config.json`

```json
{
  "providers": {
    "openrouter": {
      "apiKey": "sk-or-v1-xxx"
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5",
      "provider": "openrouter"
    }
  }
}
```

## Unterstützte LLM-Anbieter

| Anbieter | Status |
|----------|--------|
| OpenRouter | ✅ |
| Anthropic | ✅ |
| OpenAI | ✅ |
| DeepSeek | ✅ |
| Azure OpenAI | ✅ |
| Ollama (lokal) | ✅ |
| vLLM (lokal) | ✅ |
| Groq | ✅ |
| Moonshot | ✅ |
| Gemini | ✅ |
| Zhipu | ✅ |
| DashScope | ✅ |

## Funktionen

### ✅ Abgeschlossen

- **Kerninfrastruktur**: Konfigurationssystem, Anbieter-Registry, Nachrichten-Bus, CLI-Framework
- **Tool-Implementierung**: Shell-Ausführung, Dateisystem-Tools, Websuche, Web-Fetch
- **Speicher & Sitzungen**: Sitzungsspeicherung, Speicherkonsolidierung, Kontextfensterverwaltung
- **Channel-Integration**: Telegram, Discord, Feishu Channel-Connectors
- **Servicesystem**: Cron-Service, Heartbeat-Service, OpenAI-kompatibler API-Server
- **Erweiterte Funktionen**: MCP (Model Context Protocol), Subagenten-System, Skills-System

## Entwicklung

### Voraussetzungen

- Rust 1.75+ (stable)
- Tokio-Laufzeit

> **Hinweis für Ubuntu-Benutzer: protoc installieren**
>
> Bei protoc-bezogenen Fehlern während des Builds installieren Sie den Protocol Buffers-Compiler:
>
> ```bash
> # Ubuntu/Debian
> sudo apt-get update && sudo apt-get install -y protobuf-compiler
>
> # Überprüfen
> protoc --version
> ```

### Tests ausführen

```bash
cargo test
```

### Clippy ausführen

```bash
cargo clippy --all-targets
```

### Code formatieren

```bash
cargo fmt --all
```

## CLI-Befehle

| Befehl | Beschreibung |
|------|------|
| `rustbot onboard` | Konfiguration initialisieren |
| `rustbot agent -m "Nachricht"` | Einzelne Nachricht senden |
| `rustbot agent` | Interaktiver Modus |
| `rustbot api --port 8900` | API-Server starten |
| `rustbot cron list` | Cron-Jobs auflisten |
| `rustbot channels login <Channel>` | Bei Channel anmelden |
| `rustbot channels status` | Channel-Status anzeigen |
| `rustbot channels start <Channel>` | Channel starten |
| `rustbot channels stop <Channel>` | Channel stoppen |
| `rustbot mcp list` | MCP-Server auflisten |

## Lizenz

MIT-Lizenz - gleich wie das ursprüngliche nanobot-Projekt.

## Danksagungen

Dieses Projekt ist eine Rust-Neuimplementierung, inspiriert von [nanobot](https://github.com/HKUDS/nanobot), einem ultraleichten Python-KI-Assistenten-Framework.
