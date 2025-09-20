# cloudpub-common

Общая библиотека для компонентов CloudPub - платформы безопасной публикации локальных сервисов.

## О библиотеке

`cloudpub-common` содержит общий код, используемый клиентом (`cloudpub-client`), сервером (`cloudpub-server`) и графическим интерфейсом (`cloudpub-gui`). Библиотека предоставляет базовую функциональность для работы с протоколами, сетевым взаимодействием, шифрованием и логированием.

## Основные компоненты

### Безопасность и шифрование
- **TLS/SSL поддержка**: Реализация через `rustls` с нативными сертификатами
- **Управление сертификатами**: Парсинг X.509, работа с P12/PFX файлами
- **Прокси поддержка**: HTTP и SOCKS5 прокси с аутентификацией

### Сетевое взаимодействие
- **WebSocket протокол**: Асинхронная работа через `tokio-tungstenite`
- **Протокол сообщений**: Protobuf-based коммуникация между компонентами
- **Управление соединениями**: Пулы соединений, переподключения, таймауты

### Логирование и трассировка
- **Структурированное логирование**: На базе `tracing` и `tracing-subscriber`
- **Ротация логов**: Автоматическая ротация файлов логов
- **Panic обработка**: Перехват и логирование паник с backtrace

## Использование

### Добавление зависимости

```toml
[dependencies]
cloudpub-common = { version = "2.4.2", features = ["rustls"] }
```

### Примеры использования

#### Инициализация логирования

```rust
use cloudpub_common::logging;

#[tokio::main]
async fn main() {
    logging::init_tracing("info");
    tracing::info!("Приложение запущено");
}
```

#### Работа с конфигурацией

```rust
use cloudpub_common::config::Config;

let config = Config::load()?;
println!("Сервер: {}", config.server);
println!("Токен установлен: {}", config.token.is_some());
```

#### Создание защищенного соединения

```rust
use cloudpub_common::tls;

let tls_config = tls::create_client_config()?;
let connector = TlsConnector::from(Arc::new(tls_config));
```

#### Локализация

```rust
use cloudpub_common::i18n;

let message = i18n::t!("connection-established");
println!("{}", message);
```

## Архитектура

### Модули

- **`protocol`** - Определение протокола обмена сообщениями
- **`transport`** - Транспортный уровень (WebSocket, Unix sockets)
- **`tls`** - TLS/SSL функциональность
- **`config`** - Управление конфигурацией
- **`logging`** - Система логирования
- **`i18n`** - Интернационализация
- **`utils`** - Вспомогательные утилиты

### Протокол сообщений

Библиотека использует Protocol Buffers для определения сообщений:

```protobuf
message Message {
  oneof payload {
    PublishRequest publish = 1;
    PublishResponse response = 2;
    Data data = 3;
    Control control = 4;
  }
}
```

## Зависимости

### Основные
- `tokio` - Асинхронная runtime
- `bytes` - Эффективная работа с байтовыми буферами
- `futures` - Асинхронные примитивы
- `tracing` - Структурированное логирование

### Сетевые
- `tokio-tungstenite` - WebSocket клиент/сервер
- `async-http-proxy` - HTTP прокси поддержка
- `async-socks5` - SOCKS5 прокси поддержка

### Безопасность (опционально)
- `rustls` - TLS имплементация на Rust
- `rustls-native-certs` - Системные сертификаты
- `x509-parser` - Парсинг X.509 сертификатов
- `p12` - Работа с PKCS#12 файлами

## Сборка

```bash
# Сборка с базовыми возможностями
cargo build --package cloudpub-common

# Сборка с TLS поддержкой
cargo build --package cloudpub-common --features rustls

# Сборка с полной функциональностью
cargo build --package cloudpub-common --all-features
```

## Тестирование

```bash
cargo test --package cloudpub-common
```

## Версионирование

Библиотека следует семантическому версионированию (SemVer). Текущая версия: 2.4.2

## Лицензия

Apache License 2.0

## Вклад в проект

Приветствуются pull requests. Для крупных изменений сначала откройте issue для обсуждения.

## Поддержка

- [GitHub Issues](https://github.com/ermak-dev/cloudpub/issues)
- Email: support@cloudpub.ru
