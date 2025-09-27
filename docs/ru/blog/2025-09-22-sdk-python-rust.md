---
title: "CloudPub SDK для Python и Rust"
date: 2025-09-22
description: "Представляем официальные SDK для Rust и Python — автоматизируйте публикацию сервисов, управляйте туннелями программно"
tags: ["sdk", "rust", "python", "api"]
image: /img/sdk.jpg
---

# CloudPub SDK: Интеграция с проектами на Python и Rust

![SDK для CloudPub](/img/sdk.jpg)

Рады представить официальные SDK для CloudPub! Теперь вы можете интегрировать функциональность CloudPub прямо в свои приложения на Rust и Python. Больше не нужно запускать отдельный клиент — всё управление туннелями доступно программно.

## Зачем нужен SDK?

До сегодняшнего дня CloudPub работал через отдельное приложение — GUI или CLI. Это отлично для ручного управления, но что если вам нужно:

- Встроить туннели прямо в ваше приложение
- Динамически управлять доступом к сервисам
- Программно получать информцию и управлять туннелями

SDK решает все эти задачи элегантно и эффективно.

<!-- truncate -->

## Python SDK: простота и гибкость

Python SDK идеально подходит для скриптов, автоматизации и интеграции с существующими Python-приложениями.

### Установка

```bash
pip install cloudpub-python-sdk
```

### Быстрый старт

```python
from cloudpub_python_sdk import Connection, Protocol, Auth

def main():
    # Создаём соединение
    conn = Connection(
        email="user@example.com",
        password="password"
    )

    # Публикуем Django приложение
    endpoint = conn.publish(
        Protocol.HTTP,
        "localhost:8000",
        "Django App",
        Auth.NONE
    )

    print(f"✅ Приложение доступно: {endpoint.url}")

    # Держим туннель открытым
    input("Нажмите Enter для завершения...")

if __name__ == "__main__":
    main()
```

### Интеграция с фреймворками

SDK отлично интегрируется с популярными Python фреймворками:

```python
# FastAPI + CloudPub
from fastapi import FastAPI
from cloudpub_python_sdk import Connection, Protocol, Auth
import uvicorn

app = FastAPI()

@app.get("/")
def read_root():
    return {"message": "Hello from CloudPub!"}

def publish_app():
    conn = Connection(
        email="user@example.com",
        password="password"
    )

    endpoint = conn.publish(
        Protocol.HTTP,
        "localhost:8000",
        "FastAPI через CloudPub",
        Auth.NONE
    )

    print(f"API доступен: {endpoint.url}")
    return conn

if __name__ == "__main__":
    # Запускаем публикацию
    publish_app()

    # Запускаем FastAPI
    uvicorn.run(app, host="localhost", port=8000)
```

## Реальные сценарии использования

### 1. Временные туннели для тестирования WebHook

```rust
// Создаём туннель на время теста
async fn test_webhook() {
    let mut conn = Connection::builder()
        .credentials("test@example.com", "password")
        .build()
        .await?;

    // Публикуем mock-сервер
    let endpoint = conn.publish(
        Protocol::Http,
        "localhost:9999".to_string(),
        Some("Webhook Test".to_string()),
        Some(Auth::None),
        None, None, None
    ).await?;

    // Тестируем webhook
    test_external_service(endpoint.as_url()).await?;

    // Автоматически убираем туннель
    conn.unpublish(endpoint.guid).await?;
}
```

### 2. Динамическое управление доступом

```python
def update_access_rules(service_guid, blocked_ips):
    conn = Connection(
        email="admin@example.com",
        password="secure_password"
    )

    # Получаем текущий сервис
    services = conn.ls()
    service = next(s for s in services if s.guid == service_guid)

    # Обновляем правила
    new_rules = [
        {
            "name": "Блокировка IP",
            "filter": f"ip.src in {{{','.join(blocked_ips)}}}",
            "action": "block"
        }
    ]

    conn.publish(
        service.protocol,
        service.local_address,
        service.name,
        service.auth,
        rules=new_rules
    )
```

## Rust SDK: cтатическая компиляция и безопасность

Rust SDK построен на той же кодовой базе, что и основной клиент CloudPub.
Это позволяет встроить его непосредственно в бинарный файл вашего приложения, статическую типизацию, безопасность и другие преимущества характерный для языка Rust.

### Установка

Добавьте в ваш `Cargo.toml`:

```toml
[dependencies]
cloudpub-sdk = "2"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

### Простейший пример

```rust
use cloudpub_sdk::Connection;
use cloudpub_sdk::protocol::{Protocol, Auth};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Подключаемся к CloudPub
    let mut conn = Connection::builder()
        .credentials("user@example.com", "password")
        .build()
        .await?;

    // Публикуем локальный веб-сервер
    let endpoint = conn.publish(
        Protocol::Http,
        "localhost:3000".to_string(),
        Some("Dev сервер".to_string()),
        Some(Auth::None),
        None, None, None
    ).await?;

    println!("Сервис доступен по адресу: {}", endpoint.as_url());

    // Держим туннель открытым
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### Продвинутые возможности

SDK поддерживает все функции CloudPub:

```rust
// Настройка с файлом конфигурации
let mut conn = Connection::builder()
    .config_path(Path::new("/etc/cloudpub/config.toml"))
    .log_level("debug")
    .timeout_secs(30)
    .build()
    .await?;

// Публикация с контролем доступа
let endpoint = conn.publish(
    Protocol::Tcp,
    "localhost:22".to_string(),
    Some("SSH сервер".to_string()),
    Some(Auth::Password("secret123".to_string())),
    Some(vec!["192.168.1.0/24".to_string()]), // ACL
    None,
    Some(vec![
        Rule::new(
            "Блокировать подозрительные IP".to_string(),
            "ip.src in {1.2.3.4, 5.6.7.8}".to_string(),
            Action::Block
        )
    ])
).await?;

// Управление туннелями
let tunnes = conn.ls().await?;
for tunnel in tunnels {
    println!("Сервис: {} - {}", service.name, service.endpoint);

    // Остановка/запуск
    tunnel.stop(service.guid.clone()).await?;
    tunnel.start(service.guid.clone()).await?;
}

// Остановка и удаление всех сервисов
tunnels.clean().await?;
```

## Документация и примеры

Полная документация и примеры доступны:

- **Rust SDK**
  - [Документация на docs.rs](https://docs.rs/cloudpub-sdk/latest/cloudpub_sdk/)
  - [Примеры на GitHub](https://github.com/ermak-dev/cloudpub/tree/master/sdk/rust/examples)

- **Python SDK**
  - [Документация](https://cloudpub.ru/docs/python-sdk/index.html)
  - [Примеры на GitHub](https://github.com/ermak-dev/cloudpub/tree/master/sdk/python)

## Начните прямо сейчас

SDK доступны для установки через стандартные менеджеры пакетов:

```bash
# Rust
cargo add cloudpub-sdk

# Python
pip install cloudpub-python-sdk
```

Оба SDK имеют лицензию Apache 2.0 и доступны с открытым исходным кодом на [GitHub](https://github.com/ermak-dev/cloudpub).

## Что дальше?

Мы активно развиваем SDK и планируем:

- **Node.js SDK** — для JavaScript/TypeScript разработчиков
- **Go SDK** — для облачных приложений

Пробуйте новые SDK и делитесь обратной связью! Если у вас есть вопросы или предложения, присоединяйтесь к нашей [группе поддержки в Telegram](https://t.me/cloudpub_support).

---

*Команда CloudPub*
