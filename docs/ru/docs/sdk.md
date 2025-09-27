---
sidebar_position: 103
slug: /sdk
---

# SDK

Cloudpub предоставляет SDK для интеграции с различными языками программирования.

## Rust SDK

Библиотека `cloudpub-sdk` предоставляет программный интерфейс для интеграции функциональности CloudPub в ваши Rust приложения. SDK позволяет публиковать локальные сервисы, управлять туннелями и контролировать доступ программным способом.

## Установка

Добавьте в `Cargo.toml`:

```toml
[dependencies]
cloudpub-sdk = "2"
anyhow = "1"
tokio = "1
```

## Пример использования

```rust
use anyhow::Result;
use cloudpub_sdk::protocol::{Auth, Protocol};
use cloudpub_sdk::Connection;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    // Создание подключения с использованием builder pattern
    let mut conn = Connection::builder()
        .config_path(Path::new("/tmp/cloudpub.toml"))  // Путь к файлу конфигурации
        .log_level("info")                              // Уровень логирования
        .verbose(true)                                  // Вывод отладочной информации
        .credentials("user@example.com", "password")    // Учетные данные
        .timeout_secs(30)                               // Таймаут операций
        .build()
        .await?;

    // Публикация HTTP сервиса
    let endpoint = conn
        .publish(
            Protocol::Http,                         // Протокол
            "localhost:8080".to_string(),           // Локальный адрес
            Some("Мой веб-сервис".to_string()),    // Имя сервиса
            Some(Auth::None),                       // Без авторизации
        )
        .await?;

    println!("Сервис опубликован: {}", endpoint.as_url());

    // Код вашего сервиса

    Ok(())
}
```

## Документация

- [Документация](https://docs.rs/cloudpub-sdk/latest/cloudpub_sdk/)
- [Пример использования](https://github.com/ermak-dev/cloudpub/blob/master/sdk/rust/examples/example.rs)

# Python SDK

Библиотека `cloudpub-python-sdk` предоставляет программный интерфейс для интеграции функциональности CloudPub в ваши Python приложения. SDK позволяет публиковать локальные сервисы, управлять туннелями и контролировать доступ программным способом.

## Установка

```bash
pip install cloudpub-python-sdk
```

## Пример использования

```python
from cloudpub_python_sdk import Connection, Protocol, Auth

def main():
    # Создание соединения с сервером CloudPub
    # Для аутентификации должен быть предоставлен либо токен, либо email/пароль
    conn = Connection(
        "/tmp/cloudpub.toml",  # Путь к файлу конфигурации
        "info",                # Уровень логирования
        True,                  # Выводить логи в stderr
        None,                  # Токен
        "admin@example.com",   # Email
        "test"                 # Пароль
    )

    # Регистрация нового HTTP сервиса
    endpoint = conn.publish(
        Protocol.HTTP,             # Протокол
        "localhost:8080",          # Локальный адрес
        "Тестовый HTTP сервис",    # Имя сервиса
        Auth.NONE                  # Метод аутентификации для доступа к сервису
    )

    print(f"  Сервис опубликован: {endpoint.url}")

    # Код вашего сервиса...


if __name__ == "__main__":
    main()
```

## Документация

- [Python SDK](https://cloudpub.ru/docs/python-sdk/index.html)
- [Пример испольования](https://github.com/ermak-dev/cloudpub/blob/master/sdk/python/example.py)

## License

Apache License 2.0

## Поддержка

- [GitHub Issues](https://github.com/ermak-dev/cloudpub/issues)
- Email: support@cloudpub.ru
