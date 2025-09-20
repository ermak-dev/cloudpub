# cloudpub-sdk

[![Rust SDK](https://img.shields.io/crates/v/cloudpub-sdk.svg)](https://crates.io/crates/cloudpub-sdk)

Rust SDK для CloudPub - платформы безопасной публикации локальных сервисов в интернете.

## О библиотеке

`cloudpub-sdk` предоставляет программный интерфейс для интеграции функциональности CloudPub в ваши Rust приложения. SDK позволяет публиковать локальные сервисы, управлять туннелями и контролировать доступ программным способом.

## Установка

Добавьте в `Cargo.toml`:

```toml
[dependencies]
cloudpub-sdk = "2"
cloudpub-common = "2"
anyhow = "1"
tokio = "1
```

## Быстрый старт

### Пример использования

```rust
use anyhow::Result;
use cloudpub_common::protocol::{Auth, Protocol};
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
    println!("GUID сервиса: {}", endpoint.guid);

    // Получение списка всех сервисов
    let services = conn.ls().await?;
    for service in &services {
        println!("- {}: {}", service.guid, service.as_url());
    }

    // Остановка сервиса
    conn.stop(endpoint.guid.clone()).await?;
    println!("Сервис остановлен");

    // Запуск сервиса
    conn.start(endpoint.guid.clone()).await?;
    println!("Сервис запущен");

    // Удаление сервиса
    conn.unpublish(endpoint.guid.clone()).await?;
    println!("Сервис удален");

    // Очистка всех сервисов
    conn.clean().await?;
    println!("Все сервисы удалены");

    Ok(())
}
```

## API Reference

### Основные типы

#### `Connection`
Основной клиент для взаимодействия с CloudPub.

#### `Endpoint`
Представляет опубликованный сервис с информацией о доступе.

#### `Protocol`
Enum с поддерживаемыми протоколами: Http, Https, Tcp, Udp, OneC, WebDav, Minecraft.

#### `Auth`
Типы авторизации: None, Basic.

## Примеры

Пример использования доступны в директории [`examples/`](examples/):

```bash
# Запуск примера
cargo run --example example
```

## Лицензия

Apache License 2.0

## Поддержка

- [Документация](https://cloudpub.ru/docs)
- [GitHub Issues](https://github.com/ermak-dev/cloudpub/issues)
- [Пример использования](https://github.com/ermak-dev/cloudpub/blob/master/sdk/rust/examples/example.rs)
- Email: support@cloudpub.ru
