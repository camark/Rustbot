# RustBot

🐈 Framework d'assistant personnel AI ultra-léger (Implémentation Rust)

Ceci est une réécriture complète en Rust de [nanobot](https://github.com/HKUDS/nanobot), conçu pour :

- **Performance** : 10-100x plus rapide que Python
- **Faible mémoire** : Pas de GC, empreinte minimale
- **Binaire unique** : Déploiement facile, aucune dépendance
- **Sécurité de type** : Garanties à la compilation

## Structure du projet

```
RustBot/
├── Cargo.toml              # Racine du workspace
├── crates/
│   ├── nanobot-config/     # Système de configuration
│   ├── nanobot-providers/  # Fournisseurs LLM
│   ├── nanobot-bus/        # Bus de messages
│   ├── nanobot-core/       # Moteur d'agent
│   └── nanobot-cli/        # Interface CLI
└── docs/
```

## Démarrage rapide

### Compilation

```bash
cargo build --release
```

### Initialisation

```bash
./target/release/rustbot onboard
```

### Discussion

```bash
./target/release/rustbot agent -m "Bonjour !"
```

### Mode interactif

```bash
./target/release/rustbot agent
```

### Statut

```bash
./target/release/rustbot status
```

## Configuration

Fichier de configuration : `~/.nanobot/config.json`

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

## Fournisseurs LLM pris en charge

| Fournisseur | Statut |
|----------|--------|
| OpenRouter | ✅ |
| Anthropic | ✅ |
| OpenAI | ✅ |
| DeepSeek | ✅ |
| Azure OpenAI | ✅ |
| Ollama (local) | ✅ |
| vLLM (local) | ✅ |
| Groq | ✅ |
| Moonshot | ✅ |
| Gemini | ✅ |
| Zhipu | ✅ |
| DashScope | ✅ |

## Fonctionnalités

### ✅ Terminé

- **Infrastructure de base** : Système de configuration, registre des fournisseurs, bus de messages, framework CLI
- **Implémentation des outils** : Exécution shell, outils de système de fichiers, recherche web, fetch web
- **Mémoire et sessions** : Persistance des sessions, consolidation de la mémoire, gestion de la fenêtre de contexte
- **Intégration des canaux** : Connecteurs Telegram, Discord, Feishu
- **Système de services** : Service Cron, service Heartbeat, serveur API compatible OpenAI
- **Fonctionnalités avancées** : MCP (Model Context Protocol), système de sous-agents, système de compétences

## Développement

### Prérequis

- Rust 1.75+ (stable)
- Runtime Tokio

> **Note pour les utilisateurs Ubuntu : installer protoc**
>
> Si vous rencontrez des erreurs liées à protoc pendant la compilation, installez le compilateur Protocol Buffers :
>
> ```bash
> # Ubuntu/Debian
> sudo apt-get update && sudo apt-get install -y protobuf-compiler
>
> # Vérifier
> protoc --version
> ```

### Exécuter les tests

```bash
cargo test
```

### Exécuter Clippy

```bash
cargo clippy --all-targets
```

### Formater le code

```bash
cargo fmt --all
```

## Commandes CLI

| Commande | Description |
|------|------|
| `rustbot onboard` | Initialiser la configuration |
| `rustbot agent -m "message"` | Envoyer un message unique |
| `rustbot agent` | Mode interactif |
| `rustbot api --port 8900` | Démarrer le serveur API |
| `rustbot cron list` | Lister les tâches cron |
| `rustbot channels login <canal>` | Se connecter à un canal |
| `rustbot channels status` | Afficher le statut du canal |
| `rustbot channels start <canal>` | Démarrer un canal |
| `rustbot channels stop <canal>` | Arrêter un canal |
| `rustbot mcp list` | Lister les serveurs MCP |

## Licence

Licence MIT - identique au projet nanobot original.

## Remerciements

Ce projet est une réécriture en Rust inspirée par [nanobot](https://github.com/HKUDS/nanobot), un framework d'assistant AI Python ultra-léger.
