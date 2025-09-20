//! # CloudPub Rust SDK
//!
//! CloudPub Rust SDK предоставляет простой интерфейс для взаимодействия с
//! платформой CloudPub. CloudPub обеспечивает безопасное туннелирование и публикацию сервисов,
//! позволяя безопасно открывать локальные сервисы в интернет.
//!
//! ## Возможности
//!
//! - **Публикация сервисов**: Открытие локальных сервисов (HTTP, HTTPS, TCP, UDP, WebSocket, RTSP) в интернет
//! - **Безопасное туннелирование**: Все соединения шифруются и аутентифицируются
//! - **Несколько протоколов**: Поддержка различных протоколов, включая HTTP/HTTPS, TCP/UDP, WebSocket и RTSP
//! - **Аутентификация**: Встроенная поддержка аутентификации на основе токенов и учетных данных
//! - **Управление сервисами**: Запуск, остановка, список и управление опубликованными сервисами
//! - **Событийно-ориентированная архитектура**: Поддержка async/await с управлением состоянием на основе событий
//!
//! ## Быстрый старт
//!
//! Добавьте SDK в ваш `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! cloudpub-sdk = "2.4.2"
//! cloudpub-common = "2.4.2"  # Требуется для типов протокола
//! ```
//!
//! ## Базовое использование
//!
//! ### Простая публикация сервиса
//!
//! ```no_run
//! use cloudpub_sdk::Connection;
//! use cloudpub_common::protocol::{Protocol, Auth, Endpoint};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Создание соединения с учетными данными
//!     let mut conn = Connection::builder()
//!         .credentials("user@example.com", "password")
//!         .build()
//!         .await?;
//!
//!     // Публикация локального веб-сервиса
//!     let endpoint = conn.publish(
//!         Protocol::Http,
//!         "localhost:3000".to_string(),
//!         Some("My Web App".to_string()),
//!         Some(Auth::None),
//!         None, // acl
//!         None, // headers
//!         None, // rules
//!     ).await?;
//!
//!     println!("Service published at: {}", endpoint.as_url());
//!
//!     // Ожидание сигнала Ctrl+C для остановки
//!     tokio::signal::ctrl_c().await?;
//!
//!     // Очистка
//!     conn.unpublish(endpoint.guid).await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Расширенная публикация сервиса с ACL, заголовками и правилами фильтрации
//!
//! ```no_run
//! use cloudpub_sdk::Connection;
//! use cloudpub_common::protocol::{
//!     Protocol, Auth, Endpoint, Acl, Header, FilterRule,
//!     Role, FilterAction
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut conn = Connection::builder()
//!         .credentials("admin@example.com", "password")
//!         .build()
//!         .await?;
//!
//!     // Настройка списка контроля доступа
//!     let acl = vec![
//!         Acl { user: "admin@example.com".to_string(), role: Role::Admin as i32 },
//!         Acl { user: "user@example.com".to_string(), role: Role::Reader as i32 },
//!         Acl { user: "writer@example.com".to_string(), role: Role::Writer as i32 },
//!     ];
//!
//!     // Добавление пользовательских HTTP заголовков
//!     let headers = vec![
//!         Header { name: "X-API-Version".to_string(), value: "2.0".to_string() },
//!         Header { name: "Access-Control-Allow-Origin".to_string(), value: "*".to_string() },
//!         Header { name: "X-Custom-Header".to_string(), value: "CloudPub".to_string() },
//!     ];
//!
//!     // Определение правил фильтрации для обработки запросов
//!     let rules = vec![
//!         FilterRule {
//!             order: 0,
//!             action_type: FilterAction::FilterAllow as i32,
//!             action_value: None,
//!             data: "http.path starts_with \"/api/\" and http.headers[\"X-API-Key\"] contains \"valid\"".to_string(),
//!         },
//!         FilterRule {
//!             order: 1,
//!             action_type: FilterAction::FilterDeny as i32,
//!             action_value: None,
//!             data: "http.path matches \"^/admin.*\" and not (ip.src >= 192.168.1.0 and ip.src <= 192.168.1.255)".to_string(),
//!         },
//!         FilterRule {
//!             order: 2,
//!             action_type: FilterAction::FilterRedirect as i32,
//!             action_value: Some("https://api.example.com/v2/".to_string()),  // URL перенаправления
//!             data: "http.path starts_with \"/api/v1/\"".to_string(),
//!         },
//!     ];
//!
//!     // Публикация сервиса со всеми расширенными возможностями
//!     let endpoint = conn.publish(
//!         Protocol::Https,
//!         "localhost:8443".to_string(),
//!         Some("Secured API Server".to_string()),
//!         Some(Auth::Basic),
//!         Some(acl),
//!         Some(headers),
//!         Some(rules),
//!     ).await?;
//!
//!     println!("Advanced service published at: {}", endpoint.as_url());
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Расширенная конфигурация
//!
//! ```no_run
//! # async fn example() -> anyhow::Result<()> {
//! use cloudpub_sdk::Connection;
//! use std::time::Duration;
//!
//! // Создание соединения с расширенной конфигурацией
//! let conn = Connection::builder()
//!     .config_path("/custom/path/config.toml")  // Пользовательский файл конфигурации
//!     .log_level("debug")                       // Включить отладочное логирование
//!     .verbose(true)                            // Вывод логов в консоль
//!     .timeout(Duration::from_secs(60))         // Установить таймаут операции
//!     .token("existing-auth-token")             // Использовать существующий токен
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Расширенные возможности
//!
//! ### Списки контроля доступа (ACL)
//!
//! Контроль доступа к вашим сервисам на основе ролей пользователей:
//!
//! ```no_run
//! # use cloudpub_common::protocol::{Acl, Role};
//! let acl = vec![
//!     Acl {
//!         user: "admin@example.com".to_string(),
//!         role: Role::Admin as i32  // Полный доступ
//!     },
//!     Acl {
//!         user: "reader@example.com".to_string(),
//!         role: Role::Reader as i32  // Доступ только на чтение
//!     },
//!     Acl {
//!         user: "writer@example.com".to_string(),
//!         role: Role::Writer as i32  // Доступ на чтение-запись
//!     },
//!     Acl {
//!         user: "guest@example.com".to_string(),
//!         role: Role::Nobody as i32  // Нет доступа
//!     },
//! ];
//! ```
//!
//! ### Пользовательские HTTP заголовки
//!
//! Добавление пользовательских заголовков в HTTP/HTTPS ответы:
//!
//! ```no_run
//! # use cloudpub_common::protocol::Header;
//! let headers = vec![
//!     Header {
//!         name: "X-API-Version".to_string(),
//!         value: "2.0".to_string()
//!     },
//!     Header {
//!         name: "Access-Control-Allow-Origin".to_string(),
//!         value: "*".to_string()
//!     },
//!     Header {
//!         name: "Cache-Control".to_string(),
//!         value: "no-cache, no-store, must-revalidate".to_string()
//!     },
//! ];
//! ```
//!
//! ### Правила фильтрации
//!
//! Определение правил для управления маршрутизацией запросов и доступом на основе различных параметров соединения.
//! Правила обрабатываются по порядку (по полю `order`), и первое подходящее правило применяется.
//!
//! ```no_run
//! # use cloudpub_common::protocol::{FilterRule, FilterAction};
//! let rules = vec![
//!     // Разрешить доступ к API только из локальной сети
//!     FilterRule {
//!         order: 0,
//!         action_type: FilterAction::FilterAllow as i32,
//!         action_value: None,
//!         data: "ip.src >= 192.168.1.0 and ip.src <= 192.168.1.255 and http.path starts_with \"/api/\"".to_string(),
//!     },
//!
//!     // Блокировать доступ к админ-панели снаружи
//!     FilterRule {
//!         order: 1,
//!         action_type: FilterAction::FilterDeny as i32,
//!         action_value: None,
//!         data: "http.path matches \"^/admin.*\"".to_string(),
//!     },
//!
//!     // Перенаправление старых API эндпоинтов на новую версию
//!     // action_value содержит URL перенаправления для FilterRedirect
//!     FilterRule {
//!         order: 2,
//!         action_type: FilterAction::FilterRedirect as i32,
//!         action_value: Some("https://api.example.com/v2/".to_string()),  // Redirect URL
//!         data: "http.path starts_with \"/api/v1/\"".to_string(),
//!     },
//!
//!     // Блокировать запросы из определенных стран
//!     FilterRule {
//!         order: 3,
//!         action_type: FilterAction::FilterDeny as i32,
//!         action_value: None,
//!         data: "geo.country != \"Россия\"".to_string(),
//!     },
//!
//!     // Разрешить только аутентифицированные API запросы
//!     FilterRule {
//!         order: 4,
//!         action_type: FilterAction::FilterAllow as i32,
//!         action_value: None,
//!         data: "http.path starts_with \"/api/\" and http.headers[\"Authorization\"] contains \"Bearer\"".to_string(),
//!     },
//! ];
//! ```
//!
//! **Доступные переменные для фильтр выражений:**
//! - `ip.src`, `ip.dst` - IP адреса источника и назначения
//! - `port.src`, `port.dst` - Порты источника и назначения
//! - `protocol` - Протокол соединения ("tcp", "udp", "http")
//! - `http.host`, `http.path`, `http.method` - HTTP-специфичные поля
//! - `http.headers["Header-Name"]` - Доступ к HTTP заголовкам
//! - `geo.country`, `geo.region`, `geo.city`, `geo.code` - Данные геолокации
//!
//! **Операторы:**
//! - Сравнение: `==`, `!=`, `>`, `<`, `>=`, `<=`
//! - Строковые: `matches` (regex), `contains`, `starts_with`, `ends_with`
//! - Логические: `and`, `or`, `not`
//!
//! ## Типы сервисов
//!
//! CloudPub поддерживает несколько протоколов сервисов:
//!
//! ### HTTP/HTTPS сервисы
//!
//! ```no_run
//! # async fn http_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! use cloudpub_common::protocol::{Protocol, Auth, Endpoint};
//!
//! // Публикация HTTP сервиса с базовой аутентификацией
//! let http_endpoint = conn.publish(
//!     Protocol::Http,
//!     "localhost:8080".to_string(),
//!     Some("API Server".to_string()),
//!     Some(Auth::Basic),  // Требовать пароль для доступа
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//!
//! // Публикация HTTPS сервиса
//! let https_endpoint = conn.publish(
//!     Protocol::Https,
//!     "localhost:8443".to_string(),
//!     Some("Secure API".to_string()),
//!     Some(Auth::Basic),  // Требовать токен для доступа
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### TCP/UDP сервисы
//!
//! ```no_run
//! # async fn tcp_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! use cloudpub_common::protocol::{Protocol, Auth};
//!
//! // Публикация TCP сервиса (например, SSH)
//! let tcp_endpoint = conn.publish(
//!     Protocol::Tcp,
//!     "localhost:22".to_string(),
//!     Some("SSH Server".to_string()),
//!     Some(Auth::None),
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//!
//! // Публикация UDP сервиса (например, DNS)
//! let udp_endpoint = conn.publish(
//!     Protocol::Udp,
//!     "localhost:53".to_string(),
//!     Some("DNS Server".to_string()),
//!     Some(Auth::None),
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### WebSocket сервисы
//!
//! ```no_run
//! # async fn ws_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! use cloudpub_common::protocol::{Protocol, Auth};
//!
//! // Публикация TCP сервиса
//! let tcp_endpoint = conn.publish(
//!     Protocol::Tcp,
//!     "localhost:8080".to_string(),
//!     Some("TCP Server".to_string()),
//!     Some(Auth::None),
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//!
//! // Публикация UDP сервиса
//! let udp_endpoint = conn.publish(
//!     Protocol::Udp,
//!     "localhost:8443".to_string(),
//!     Some("UDP Server".to_string()),
//!     Some(Auth::Basic),
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### RTSP стриминговые сервисы
//!
//! ```no_run
//! # async fn rtsp_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! use cloudpub_common::protocol::{Protocol, Auth};
//!
//! // Публикация RTSP потока с учетными данными в URL
//! // Формат: rtsp://username:password@host:port/path
//! let rtsp_endpoint = conn.publish(
//!     Protocol::Rtsp,
//!     "rtsp://camera:secret123@192.168.1.100:554/live/stream1".to_string(),
//!     Some("Security Camera".to_string()),
//!     Some(Auth::Basic),  // Аутентификация доступа в CloudPub
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//!
//! // RTSP без учетных данных (публичный поток)
//! let public_rtsp = conn.publish(
//!     Protocol::Rtsp,
//!     "rtsp://localhost:554/public".to_string(),
//!     Some("Public Stream".to_string()),
//!     Some(Auth::None),
//!     None, // acl
//!     None, // headers
//!     None, // rules
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Управление сервисами
//!
//! ### Список сервисов
//!
//! ```no_run
//! # async fn list_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! use cloudpub_common::protocol::Endpoint;
//! // Получить все зарегистрированные сервисы
//! let services = conn.ls().await?;
//!
//! for service in services {
//!     let name = service.client.as_ref()
//!         .and_then(|c| c.description.clone())
//!         .unwrap_or_else(|| "Без имени".to_string());
//!     println!("Service: {}", name);
//!     println!("  GUID: {}", service.guid);
//!     println!("  URL: {}", service.as_url());
//!     println!("  Status: {}", service.status.unwrap_or_else(|| "Unknown".to_string()));
//!     println!("  Protocol: {:?}", service.remote_proto);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Запуск и остановка сервисов
//!
//! ```no_run
//! # async fn control_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! // Временно остановить сервис
//! conn.stop("service-guid-123".to_string()).await?;
//!
//! // Перезапустить сервис
//! conn.start("service-guid-123".to_string()).await?;
//!
//! // Постоянно удалить сервис
//! conn.unpublish("service-guid-123".to_string()).await?;
//!
//! // Удалить все сервисы
//! conn.clean().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Управление конфигурацией
//!
//! ### Чтение и запись опций
//!
//! ```no_run
//! # fn config_example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! // Установка значений конфигурации
//! conn.set("server", "api.cloudpub.com")?;
//! conn.set("port", "443")?;
//! conn.set("ssl", "true")?;
//!
//! // Получение значений конфигурации
//! let server = conn.get("server")?;
//! println!("Server: {}", server);
//!
//! // Получение всех опций конфигурации
//! let options = conn.options();
//! for (key, value) in options {
//!     println!("{}: {}", key, value);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Управление аутентификацией
//!
//! ```no_run
//! # async fn auth_example() -> anyhow::Result<()> {
//! use cloudpub_sdk::Connection;
//!
//! // Аутентификация с учетными данными
//! let mut conn = Connection::builder()
//!     .credentials("user@example.com", "password")
//!     .build()
//!     .await?;
//!
//! // Позже, выход для очистки токена
//! conn.logout()?;
//!
//! // Повторная аутентификация с сохраненным токеном
//! let conn = Connection::builder()
//!     .token("saved-auth-token")
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Вспомогательные функции
//!
//! ### Проверка состояния сервера
//!
//! ```no_run
//! # async fn ping_example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! // Измерение задержки сервера (возвращает микросекунды)
//! let latency_us = conn.ping().await?;
//! let latency_ms = latency_us as f64 / 1000.0;
//! println!("Server latency: {}μs ({:.2}ms)", latency_us, latency_ms);
//!
//! if latency_ms > 100.0 {
//!     println!("Warning: High latency detected!");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Управление кэшем
//!
//! ```no_run
//! # fn cache_example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
//! // Очистка локального кэша
//! conn.purge()?;
//! println!("Cache cleared");
//! # Ok(())
//! # }
//! ```
//!
//! ## Обработка ошибок
//!
//! Все методы SDK возвращают `Result<T>` для правильной обработки ошибок:
//!
//! ```no_run
//! # async fn error_example() -> anyhow::Result<()> {
//! use cloudpub_sdk::Connection;
//!
//! match Connection::builder()
//!     .credentials("user@example.com", "wrong-password")
//!     .build()
//!     .await
//! {
//!     Ok(conn) => {
//!         println!("Connected successfully");
//!     },
//!     Err(e) => {
//!         eprintln!("Connection failed: {}", e);
//!         // Обработка специфичных типов ошибок
//!         if e.to_string().contains("authentication") {
//!             eprintln!("Authentication failed. Please check your credentials.");
//!         }
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Полный пример
//!
//! Вот полный пример, демонстрирующий полный жизненный цикл с расширенными возможностями:
//!
//! ```no_run
//! use cloudpub_sdk::Connection;
//! use cloudpub_common::protocol::{
//!     Protocol, Auth, Endpoint, Acl, Header, FilterRule,
//!     Role, FilterAction
//! };
//! use std::time::Duration;
//! use tokio::time::sleep;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Инициализация соединения с конфигурацией
//!     println!("Connecting to CloudPub...");
//!     let mut conn = Connection::builder()
//!         .credentials("admin@example.com", "secure-password")
//!         .log_level("info")
//!         .verbose(true)
//!         .timeout_secs(30)
//!         .build()
//!         .await?;
//!
//!     println!("Connected successfully!");
//!
//!     // Публикация нескольких сервисов
//!     println!("\nPublishing services...");
//!
//!     // Простой веб-сервис без расширенных возможностей
//!     let web_service = conn.publish(
//!         Protocol::Http,
//!         "localhost:3000".to_string(),
//!         Some("Web Application".to_string()),
//!         Some(Auth::None),
//!         None, // Нет ACL ограничений
//!         None, // Нет пользовательских заголовков
//!         None, // Нет правил фильтрации
//!     ).await?;
//!     println!("✓ Web app: {}", web_service.as_url());
//!
//!     // API сервер с ACL и пользовательскими заголовками
//!     let api_acl = vec![
//!         Acl { user: "api_admin@example.com".to_string(), role: Role::Admin as i32 },
//!         Acl { user: "api_user@example.com".to_string(), role: Role::Reader as i32 },
//!     ];
//!     let api_headers = vec![
//!         Header { name: "X-API-Version".to_string(), value: "2.0".to_string() },
//!         Header { name: "Access-Control-Allow-Origin".to_string(), value: "*".to_string() },
//!     ];
//!     let api_service = conn.publish(
//!         Protocol::Https,
//!         "localhost:8443".to_string(),
//!         Some("API Server".to_string()),
//!         Some(Auth::Basic),
//!         Some(api_acl),
//!         Some(api_headers),
//!         None, // Нет правил фильтрации
//!     ).await?;
//!     println!("✓ API server: {}", api_service.as_url());
//!
//!     // SSH сервис с правилами фильтрации для безопасности
//!     let ssh_rules = vec![
//!         FilterRule {
//!             order: 0,
//!             action_type: FilterAction::FilterAllow as i32,
//!             action_value: None,
//!             data: "ip.src == 192.168.1.10 or ip.src == 10.0.0.5".to_string(),  // Разрешить только определенные IP
//!         },
//!         FilterRule {
//!             order: 1,
//!             action_type: FilterAction::FilterDeny as i32,
//!             action_value: None,
//!             data: "ip.src matches \".*\"".to_string(),  // Заблокировать все остальные IP
//!         },
//!     ];
//!     let ssh_service = conn.publish(
//!         Protocol::Tcp,
//!         "localhost:22".to_string(),
//!         Some("SSH Access".to_string()),
//!         Some(Auth::Basic),
//!         None, // Нет ACL
//!         None, // Нет заголовков (TCP не использует HTTP заголовки)
//!         Some(ssh_rules),
//!     ).await?;
//!     println!("✓ SSH: {}", ssh_service.as_url());
//!
//!     // Проверка состояния сервера
//!     println!("\nChecking server health...");
//!     let latency_us = conn.ping().await?;
//!     println!("Server latency: {}μs ({:.2}ms)", latency_us, latency_us as f64 / 1000.0);
//!
//!     // Список всех сервисов
//!     println!("\nActive services:");
//!     let services = conn.ls().await?;
//!     for (i, service) in services.iter().enumerate() {
//!         let name = service.client.as_ref()
//!             .and_then(|c| c.description.clone())
//!             .unwrap_or_else(|| "Без имени".to_string());
//!         println!("{}. {} - {}",
//!             i + 1,
//!             name,
//!             service.as_url()
//!         );
//!     }
//!
//!     // Работать некоторое время
//!     println!("\nServices are running. Press Ctrl+C to stop...");
//!     tokio::select! {
//!         _ = tokio::signal::ctrl_c() => {
//!             println!("\nShutting down...");
//!         }
//!         _ = sleep(Duration::from_secs(3600)) => {
//!             println!("\nTimeout reached");
//!         }
//!     }
//!
//!     // Очистка
//!     println!("Очистка сервисов...");
//!     for service in services {
//!         conn.unpublish(service.guid).await?;
//!         let name = service.client.as_ref()
//!             .and_then(|c| c.description.clone())
//!             .unwrap_or_else(|| "Без имени".to_string());
//!         println!("✓ Removed: {}", name);
//!     }
//!
//!     println!("All services stopped. Goodbye!");
//!     Ok(())
//! }
//! ```
//! ## Потокобезопасность
//!
//! Структура `Connection` использует внутреннюю синхронизацию и может безопасно использоваться
//! между потоками через `Arc<Mutex<Connection>>` при необходимости.
//!
//! ## Лицензия
//!
//! Этот SDK лицензирован под лицензией Apache 2.0.

pub use cloudpub_client::config::ClientOpts;

mod builder;
mod connection;

pub use builder::ConnectionBuilder;
pub use connection::{CheckSignalFn, Connection, ConnectionEvent};
