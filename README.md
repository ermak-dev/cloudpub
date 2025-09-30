# CloudPub

Платформа безопасной публикации локальных сервисов в интернете. Открытый клиент под лицензией Apache 2.0.

 - Сайт проекта: https://cloudpub.ru
 - Документация: https://cloudpub.ru/docs

[![Звезды на GitHub](https://img.shields.io/github/stars/ermak-dev/cloudpub)](https://github.com/ermak-dev/cloudpub/stargazers)
[![Лицензия](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Образ для Docker](https://img.shields.io/docker/pulls/cloudpub/cloudpub)](https://hub.docker.com/r/cloudpub/cloudpub)
[![Rust SDK](https://img.shields.io/crates/v/cloudpub-sdk.svg)](https://crates.io/crates/cloudpub-sdk)
[![Python SDK](https://img.shields.io/pypi/v/cloudpub-python-sdk)](https://pypi.org/project/cloudpub-python-sdk/)

## Что такое CloudPub

CloudPub – это отечественная альтернатива Ngrok, представляющая собой комбинацию прокси-сервера, шлюза и туннеля в локальную сеть. Его основная задача заключается в предоставлении публичного доступа к локальным ресурсам через защищенный канал.

### Возможности

- **Поддержка множества протоколов**: HTTP, HTTPS, TCP, UDP, 1C, WebDAV, Minecraft, RTSP
- **Плагинная архитектура**: Легкое добавление новых протоколов
- **Безопасность**: Шифрование трафика через TLS, управление доступом
- **Простота**: Публикация сервиса одной командой
- **Кроссплатформенность**: Windows, Linux, macOS, Android (Termux)
- **Автозапуск**: Установка в качестве системного сервиса
- **API и SDK**: Программная интеграция для Rust и Python

## Компоненты проекта

- **[client/](client/)** - CLI клиент (`clo`) для управления публикациями
- **[common/](common/)** - Общие компоненты и описание протокола
- **[sdk/rust](sdk/rust)** - SDK для Rust
- **[sdk/python](sdk/python)** - SDK для Python

## Быстрый старт

### Установка

#### Готовые бинарные файлы

Скачайте последнюю версию с [cloudpub.ru](https://cloudpub.ru/docs) для вашей платформы.

#### Из исходного кода

```bash
cargo build --release --package cloudpub-client
```

#### Из исходного кода для Android с помощью `cargo-ndk`

```bash
cargo ndk build --platform 24 --release --package cloudpub-client
```

#### Установка через Cargo

```bash
cargo install cloudpub-client
```

### Использование

#### 1. Регистрация и авторизация

```bash
# Создайте аккаунт на https://cloudpub.ru/dashboard
# Затем авторизуйтесь через CLI:
clo login
```

#### 2. Публикация сервиса

```bash
# HTTP сервер на порту 8080
clo publish http 8080

# С именем и авторизацией
clo publish -n "Мой сервис" -a basic http 8080

# TCP сервис (например, база данных)
clo publish tcp 5432

# Файлы (через WebDAV)
clo publish webdav /path/to/files
```

#### 3. Управление публикациями

```bash
# Список активных публикаций
clo ls

# Остановить публикацию
clo stop <guid>

# Удалить публикацию
clo unpublish <guid>
```

## Основные команды

| Команда | Описание |
|---------|----------|
| `login` | Авторизация на сервере |
| `logout` | Завершение сессии |
| `publish` | Добавить и запустить публикацию |
| `unpublish` | Удалить публикацию |
| `ls` | Список публикаций |
| `run` | Запустить агент с сохраненными публикациями |
| `start/stop` | Управление конкретной публикацией |
| `clean` | Удалить все публикации |
| `service` | Управление системным сервисом |
| `upgrade` | Обновить клиент |

## Примеры

### HTTP/HTTPS сервисы

```bash
# Локальный веб-сервер
clo publish http 3000

# HTTPS на другом хосте
clo publish https 192.168.1.100:443

# С заголовками
clo publish -H "X-Custom:value" http 8080
```

### Управление доступом

```bash
# Базовая авторизация
clo publish -a basic http 8080

# ACL правила (email:role)
clo publish -a form -A admin@example.com:admin -A user@example.com:reader http 8080
```

### Системный сервис

```bash
# Установить (требует sudo)
sudo clo service install

# Запустить
sudo clo service start

# Статус
clo service status
```

## SDK

### Rust SDK

```rust
use cloudpub_sdk::Connection;
use cloudpub_common::protocol::{Protocol, Auth};

#[tokio::main]
async fn main() -> Result<()> {
    let mut conn = Connection::builder()
        .credentials("user@example.com", "password")
        .build()
        .await?;

    let endpoint = conn.publish(
        Protocol::Http,
        "localhost:8080".to_string(),
        Some("Мой сервис".to_string()),
        Some(Auth::None),
    ).await?;

    println!("Опубликовано: {}", endpoint.as_url());
    Ok(())
}
```

 - [Полная документация](https://docs.rs/cloudpub-sdk/latest/cloudpub_sdk/)

### Python SDK

```python
from cloudpub_python_sdk import Connection, Protocol, Auth

# Создание подключения
conn = Connection(
    config_path="/tmp/cloudpub.toml",
    log_level="info",
    verbose=True,
    email="user@example.com",
    password="password"
)

# Публикация HTTP сервиса
endpoint = conn.publish(
    Protocol.HTTP,
    "localhost:8080",
    "Мой сервис",
    Auth.NONE
)

print(f"Опубликовано: {endpoint.url}")

# Управление сервисами
services = conn.ls()
conn.start(endpoint.guid)
conn.stop(endpoint.guid)
conn.unpublish(endpoint.guid)
```

 - [Полная документация](https://cloudpub.ru/docs/python-sdk/index.html)

## Сборка

### Docker (все архитектуры)

```bash
docker build --target artifacts --output type=local,dest=. .
```

Результат в директории `artifacts`:
- `x86_64/clo` - Linux x86_64
- `aarch64/clo` - Linux ARM64
- `arm/clo` - Linux ARM
- `win64/clo.exe` - Windows x86_64

### Локальная сборка

```bash
# CLI клиент
cargo build -p cloudpub-client --release

# SDK
cargo build -p cloudpub-sdk --release
```

## Документация

- [Официальная документация](https://cloudpub.ru/docs)
- [Rust SDK](https://docs.rs/cloudpub-sdk/latest/cloudpub_sdk/)
- [Python SDK](https://cloudpub.ru/docs/python-sdk/index.html)

## Лицензия

Apache License 2.0

## Поддержка

- [GitHub Issues](https://github.com/ermak-dev/cloudpub/issues)
- Email: support@cloudpub.ru
- Telegram: [@cloudpub_support](https://t.me/cloudpub_support)

## Благодарности

На основе [RatHole](https://github.com/rapiz1/rathole) от [Юджиа Цяо](https://github.com/rapiz1)
