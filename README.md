# ADI Plugin Registry

HTTP server for hosting and distributing plugins and packages.

## Quick Start

### Docker (Recommended)

```bash
docker run -d \
  --name adi-plugin-registry \
  -p 8080:8080 \
  -v registry-data:/data \
  -e RUST_LOG=info \
  ghcr.io/mgorunuch/adi-plugin-registry:latest
```

### Docker Compose

```yaml
services:
  plugin-registry:
    image: ghcr.io/mgorunuch/adi-plugin-registry:latest
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - registry-data:/data
    environment:
      - RUST_LOG=info

volumes:
  registry-data:
```

```bash
docker-compose up -d
```

### From Source

```bash
cargo build --release -p adi-plugin-registry-http
./target/release/adi-plugin-registry /path/to/data
```

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PORT` | `8080` | HTTP server port |
| `REGISTRY_DATA_DIR` | `./registry-data` | Data storage directory |
| `RUST_LOG` | `info` | Log level (error, warn, info, debug, trace) |

## API Reference

### Health Check

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "ok",
  "service": "adi-plugin-registry",
  "version": "0.8.3"
}
```

### Get Registry Index

Returns all packages and plugins in the registry.

```bash
curl http://localhost:8080/v1/index.json
```

Response:
```json
{
  "version": 1,
  "updated_at": 1702900000,
  "packages": [...],
  "plugins": [...]
}
```

### Search

Search for packages and plugins by name, description, or tags.

```bash
# Search all
curl "http://localhost:8080/v1/search?q=theme"

# Search packages only
curl "http://localhost:8080/v1/search?q=theme&kind=package"

# Search plugins only
curl "http://localhost:8080/v1/search?q=theme&kind=plugin"
```

Response:
```json
{
  "packages": [...],
  "plugins": [...]
}
```

### Plugins

#### Get Latest Plugin Version

```bash
curl http://localhost:8080/v1/plugins/{plugin-id}/latest.json
```

#### Get Specific Plugin Version

```bash
curl http://localhost:8080/v1/plugins/{plugin-id}/{version}.json
```

Response:
```json
{
  "id": "my.plugin",
  "version": "1.0.0",
  "platforms": [
    {
      "platform": "darwin-aarch64",
      "download_url": "/v1/plugins/my.plugin/1.0.0/darwin-aarch64.tar.gz",
      "size_bytes": 1024,
      "checksum": "sha256...",
      "signature": null
    }
  ],
  "published_at": 1702900000
}
```

#### Download Plugin

```bash
curl -O http://localhost:8080/v1/plugins/{plugin-id}/{version}/{platform}.tar.gz
```

Supported platforms:
- `darwin-aarch64` (macOS Apple Silicon)
- `darwin-x86_64` (macOS Intel)
- `linux-x86_64` (Linux 64-bit)
- `linux-aarch64` (Linux ARM64)
- `windows-x86_64` (Windows 64-bit)

#### Publish Plugin

```bash
curl -X POST \
  "http://localhost:8080/v1/publish/plugins/{plugin-id}/{version}/{platform}?name=My+Plugin&description=A+cool+plugin&plugin_type=theme&author=yourname&tags=ui,theme" \
  -F "file=@plugin.tar.gz"
```

Query parameters:
| Parameter | Required | Description |
|-----------|----------|-------------|
| `name` | Yes | Display name |
| `description` | No | Plugin description |
| `plugin_type` | No | Type: theme, extension, font, etc. (default: extension) |
| `author` | No | Author name |
| `tags` | No | Comma-separated tags |

### Packages

Packages work the same as plugins but use `/v1/packages/` endpoints:

```bash
# Get latest
curl http://localhost:8080/v1/packages/{package-id}/latest.json

# Get specific version
curl http://localhost:8080/v1/packages/{package-id}/{version}.json

# Download
curl -O http://localhost:8080/v1/packages/{package-id}/{version}/{platform}.tar.gz

# Publish
curl -X POST \
  "http://localhost:8080/v1/publish/packages/{package-id}/{version}/{platform}?name=My+Package&description=Description&author=yourname" \
  -F "file=@package.tar.gz"
```

## Usage Examples

### Publishing a Plugin

1. Create your plugin tarball:
```bash
mkdir my-plugin
echo '{"name": "my-plugin", "version": "1.0.0"}' > my-plugin/manifest.json
# Add your plugin files...
tar -czf my-plugin.tar.gz my-plugin
```

2. Publish to registry:
```bash
curl -X POST \
  "http://localhost:8080/v1/publish/plugins/com.example.my-plugin/1.0.0/darwin-aarch64?name=My+Plugin&description=An+awesome+plugin&plugin_type=extension&author=developer" \
  -F "file=@my-plugin.tar.gz"
```

3. Verify publication:
```bash
curl http://localhost:8080/v1/plugins/com.example.my-plugin/latest.json
```

### Downloading a Plugin

```bash
# Get plugin info
curl http://localhost:8080/v1/plugins/com.example.my-plugin/latest.json

# Download
curl -O http://localhost:8080/v1/plugins/com.example.my-plugin/1.0.0/darwin-aarch64.tar.gz

# Extract
tar -xzf darwin-aarch64.tar.gz
```

### Using with lib-plugin-registry Client

```rust
use lib_plugin_registry::{RegistryClient, SearchKind};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RegistryClient::new("http://localhost:8080")
        .with_cache(PathBuf::from("~/.cache/plugins"));

    // Search for plugins
    let results = client.search("theme", SearchKind::PluginsOnly).await?;
    for plugin in results.plugins {
        println!("{}: {}", plugin.id, plugin.name);
    }

    // Download a plugin
    let bytes = client.download_plugin(
        "com.example.theme",
        "1.0.0",
        "darwin-aarch64",
        |done, total| println!("Progress: {}/{}", done, total)
    ).await?;

    // Save to file
    std::fs::write("theme.tar.gz", bytes)?;

    Ok(())
}
```

## Production Deployment

### With Nginx Reverse Proxy

```nginx
server {
    listen 443 ssl;
    server_name plugins.example.com;

    ssl_certificate /etc/letsencrypt/live/plugins.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/plugins.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # For large plugin uploads
        client_max_body_size 100M;
    }
}
```

### With Docker Compose + Traefik

```yaml
services:
  plugin-registry:
    image: ghcr.io/mgorunuch/adi-plugin-registry:latest
    restart: unless-stopped
    volumes:
      - registry-data:/data
    environment:
      - RUST_LOG=info
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.registry.rule=Host(`plugins.example.com`)"
      - "traefik.http.routers.registry.tls.certresolver=letsencrypt"
      - "traefik.http.services.registry.loadbalancer.server.port=8080"

volumes:
  registry-data:
```

### Backup

The registry stores all data in the `/data` volume:

```bash
# Backup
docker run --rm -v registry-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/registry-backup.tar.gz /data

# Restore
docker run --rm -v registry-data:/data -v $(pwd):/backup alpine \
  tar xzf /backup/registry-backup.tar.gz -C /
```

## Data Structure

```
/data
├── index.json           # Registry index
├── packages/
│   └── {package-id}/
│       └── {version}/
│           ├── info.json
│           └── {platform}.tar.gz
└── plugins/
    └── {plugin-id}/
        └── {version}/
            ├── info.json
            └── {platform}.tar.gz
```

## License

BSL-1.0
