use anyhow::{bail, Result};
use cloudpub_client::commands::PublishArgs;
pub use cloudpub_client::config::ClientConfig;
use cloudpub_client::ping;
use cloudpub_client::shell::get_cache_dir;
use cloudpub_common::logging::WorkerGuard;
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::{
    Break, EndpointClear, EndpointList, EndpointRemove, EndpointStart, EndpointStartAll,
    EndpointStop, ServerEndpoint,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{sleep, timeout};
use tracing::debug;

use crate::builder::ConnectionBuilder;

/// Псевдоним типа для функции проверки сигналов.
///
/// Эта функция вызывается периодически для проверки сигналов прерывания
/// (например, Ctrl+C в привязках Python). Возвращает ошибку для прерывания операции.
///
/// # Пример
/// ```no_run
/// use std::sync::Arc;
/// use anyhow::Result;
/// use cloudpub_sdk::CheckSignalFn;
///
/// fn interrupted() -> bool {
///     // Ваша логика проверки прерывания здесь
///     false
/// }
///
/// let check_signal: CheckSignalFn = Arc::new(|| {
///     if interrupted() {
///         anyhow::bail!("Операция прервана пользователем")
///     }
///     Ok(())
/// });
/// ```
pub type CheckSignalFn = Arc<dyn Fn() -> Result<()> + Send + Sync>;

/// Представляет различные события, происходящие в течение жизненного цикла соединения.
///
/// События генерируются асинхронно при изменении состояния соединения или
/// в ответ на операции. Используйте `Connection::wait_for_event()` для ожидания
/// конкретных событий.
///
/// # Поток событий
///
/// 1. `Idle` → Начальное состояние (редко видно пользователям)
/// 2. `Authenticating` → Когда начинается аутентификация
/// 3. `Connected` → Успешно аутентифицирован и готов к операциям
/// 4. `Endpoint` → Ответ на операции с конечной точкой (register, publish)
/// 5. `List` → Ответ на операции листинга
/// 6. `Acknowledged` → Операция успешно завершена
/// 7. `Error` → Операция завершилась с ошибкой
/// 8. `Closed` → Соединение прервано
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionEvent {
    /// Начальное состояние перед установлением соединения.
    /// Это состояние обычно не наблюдается пользователями.
    #[allow(dead_code)]
    Idle,
    /// Идет аутентификация.
    /// Это событие генерируется, когда клиент начинает аутентификацию с сервером.
    Authenticating,
    /// Успешно аутентифицирован и подключен к серверу.
    /// После этого события соединение готово к операциям.
    Connected,
    /// Результат операции конечной точки (register, publish, ping).
    /// Содержит детали конечной точки, включая ее GUID и URL.
    Endpoint(Box<ServerEndpoint>),
    /// Результат операции листинга.
    /// Содержит все зарегистрированные конечные точки с их текущим статусом.
    List(Vec<ServerEndpoint>),
    /// Операция была подтверждена сервером.
    /// Генерируется для операций, таких как stop, unpublish и clean.
    Acknowledged,
    /// Произошла ошибка во время операции.
    /// Содержит сообщение об ошибке от сервера.
    Error(String),
    /// Соединение было закрыто.
    /// Это может произойти из-за проблем с сетью, отключения сервера или явного разрыва соединения.
    Closed,
}

/// Управляет соединением с сервером CloudPub.
///
/// Структура `Connection` предоставляет высокоуровневый интерфейс для взаимодействия с
/// сервером CloudPub, обработки аутентификации, регистрации сервисов и
/// управления жизненным циклом.
///
/// # Пример
///
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// use cloudpub_sdk::Connection;
/// use cloudpub_common::protocol::{Protocol, Auth, Endpoint};
///
/// // Создание и настройка соединения
/// let mut conn = Connection::builder()
///     .credentials("user@example.com", "password")
///     .timeout_secs(30)
///     .build()
///     .await?;
///
/// // Регистрация сервиса
/// let endpoint = conn.publish(
///     Protocol::Http,
///     "localhost:8080".to_string(),
///     Some("My Service".to_string()),
///     Some(Auth::None),
///     None,
///     None,
///     None,
/// ).await?;
///
/// println!("Сервис доступен по адресу: {}", endpoint.as_url());
///
/// // Список всех сервисов
/// let services = conn.ls().await?;
/// for service in services {
///     let name = service.client.as_ref()
///         .and_then(|c| c.description.clone())
///         .unwrap_or_else(|| "Безымянный".to_string());
///     println!("Сервис: {} - {}", name, service.as_url());
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Потокобезопасность
///
/// `Connection` использует внутреннюю синхронизацию и может быть безопасно разделен между
/// потоками с использованием `Arc<Mutex<Connection>>` при необходимости.
pub struct Connection {
    pub config: Arc<RwLock<ClientConfig>>,
    pub command_tx: mpsc::Sender<Message>,
    pub(crate) event_tx: broadcast::Sender<ConnectionEvent>,
    pub(crate) receiver_handle: Option<tokio::task::JoinHandle<()>>,
    pub(crate) timeout: Arc<Mutex<Duration>>,
    pub(crate) check_signal_fn: Option<CheckSignalFn>,
    pub(crate) _worker_guard: WorkerGuard,
    pub(crate) client_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Connection {
    /// Создает новый построитель соединения для настройки параметров соединения.
    ///
    /// Это рекомендуемый способ создания нового соединения.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let conn = Connection::builder()
    ///     .config_path("/path/to/config.toml")
    ///     .credentials("user@example.com", "password")
    ///     .timeout_secs(30)
    ///     .verbose(true)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> ConnectionBuilder {
        ConnectionBuilder::new()
    }

    /// Создает новое соединение с управлением событиями
    pub(crate) fn new(
        config: Arc<RwLock<ClientConfig>>,
        command_tx: mpsc::Sender<Message>,
        mut result_rx: mpsc::Receiver<Message>,
        timeout: Duration,
        check_signal_fn: Option<CheckSignalFn>,
        worker_guard: WorkerGuard,
        client_handle: Option<tokio::task::JoinHandle<()>>,
    ) -> Self {
        // Create broadcast channel for event changes with a reasonable buffer
        let (event_tx, _) = broadcast::channel(100);
        let event_tx_clone = event_tx.clone();

        // Start the receiver loop in a separate task
        let command_tx_clone = command_tx.clone();
        let receiver_handle = tokio::spawn(async move {
            while let Some(msg) = result_rx.recv().await {
                debug!("Received message: {:?}", msg);
                let new_event = match msg {
                    Message::ConnectState(st) => {
                        if st == cloudpub_common::protocol::ConnectState::Connected as i32 {
                            command_tx_clone
                                .send(Message::EndpointStartAll(EndpointStartAll {}))
                                .await
                                .ok();
                            ConnectionEvent::Connected
                        } else if st == cloudpub_common::protocol::ConnectState::Disconnected as i32
                        {
                            ConnectionEvent::Closed
                        } else {
                            continue;
                        }
                    }
                    Message::EndpointAck(endpoint) => ConnectionEvent::Endpoint(Box::new(endpoint)),
                    Message::EndpointListAck(list) => ConnectionEvent::List(list.endpoints),
                    Message::EndpointStopAck(_)
                    | Message::EndpointRemoveAck(_)
                    | Message::EndpointClearAck(_) => ConnectionEvent::Acknowledged,
                    Message::Error(err) => ConnectionEvent::Error(err.message),
                    Message::Break(_) => ConnectionEvent::Closed,
                    _ => continue, // Игнорировать другие сообщения
                };

                // Отправить событие через широковещательный канал
                debug!("New event: {:?}", new_event);
                // Игнорировать ошибки отправки (нет получателей)
                let _ = event_tx_clone.send(new_event);
            }
        });

        Connection {
            config,
            command_tx,
            event_tx,
            receiver_handle: Some(receiver_handle),
            timeout: Arc::new(Mutex::new(timeout)),
            check_signal_fn,
            _worker_guard: worker_guard,
            client_handle,
        }
    }

    /// Ожидает наступления определенного события с таймаутом.
    ///
    /// Этот метод блокируется до получения целевого события или истечения таймаута.
    /// Он используется внутренне другими методами, но также может использоваться напрямую для
    /// пользовательской обработки событий.
    ///
    /// # Аргументы
    ///
    /// * `target_event` - Предикатная функция, которая возвращает true при получении желаемого события
    ///
    /// # Возвращает
    ///
    /// Возвращает соответствующее событие или ошибку, если:
    /// - Истек таймаут
    /// - Соединение закрыто
    /// - Получено событие ошибки
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// use cloudpub_sdk::ConnectionEvent;
    ///
    /// // Ожидать установки соединения
    /// let event = conn.wait_for_event(|e| matches!(e, ConnectionEvent::Connected)).await?;
    ///
    /// // Ожидать любой операции с эндпоинтом
    /// let event = conn.wait_for_event(|e| matches!(e, ConnectionEvent::Endpoint(_))).await?;
    /// if let ConnectionEvent::Endpoint(endpoint) = event {
    ///     println!("Эндпоинт зарегистрирован: {}", endpoint.guid);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn wait_for_event(
        &self,
        target_event: impl Fn(&ConnectionEvent) -> bool + Send,
    ) -> Result<ConnectionEvent> {
        let timeout_duration = *self.timeout.lock().await;
        let check_signal = self.check_signal_fn.clone();

        // Создать нового подписчика для каждого вызова wait_for_event
        let mut event_rx = self.event_tx.subscribe();

        timeout(timeout_duration, async {
            loop {
                // Попытаться получить изменения событий с опциональной проверкой сигнала
                let current_event = if let Some(ref check_fn) = check_signal {
                    // Использовать tokio::select для ожидания изменения события или таймаута для проверки сигнала
                    tokio::select! {
                        Ok(event) = event_rx.recv() => {
                            debug!("Received event: {:?}", event);
                            event
                        }
                        _ = sleep(Duration::from_millis(100)) => {
                            // Проверять сигнал каждые 100мс
                            check_fn()?;
                            // После проверки сигнала продолжить ожидание
                            continue;
                        }
                    }
                } else {
                    // Без проверки сигнала, просто ожидать событие
                    match event_rx.recv().await {
                        Ok(event) => {
                            debug!("Received event: {:?}", event);
                            event
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            bail!("Канал событий закрыт");
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Некоторые сообщения были потеряны, продолжить
                            continue;
                        }
                    }
                };

                // Проверить, находимся ли мы в событии ошибки
                if let ConnectionEvent::Error(ref msg) = current_event {
                    bail!("Операция не удалась: {}", msg);
                }

                // Проверить, находимся ли мы в событии закрытия
                if current_event == ConnectionEvent::Closed {
                    bail!("Соединение закрыто");
                }

                // Проверить, достигли ли мы целевого события
                if target_event(&current_event) {
                    return Ok(current_event);
                }

                // Событие не соответствует, продолжить чтение из канала
                debug!("Событие не соответствует целевому, продолжаем...");
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("Таймаут ожидания события"))?
    }

    /// Устанавливает значение конфигурации.
    ///
    /// # Аргументы
    ///
    /// * `key` - Ключ конфигурации (например, "server", "port", "ssl")
    /// * `value` - Значение конфигурации
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # fn example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// conn.set("server", "api.example.com")?;
    /// conn.set("port", "443")?;
    /// conn.set("ssl", "true")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.config.write().set(key, value)?;
        Ok(())
    }

    /// Получает значение конфигурации.
    ///
    /// # Аргументы
    ///
    /// * `key` - Ключ конфигурации для получения
    ///
    /// # Возвращает
    ///
    /// Значение конфигурации или ошибку, если ключ не существует.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # fn example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// let server = conn.get("server")?;
    /// println!("Подключено к серверу: {}", server);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self, key: &str) -> Result<String> {
        self.config.read().get(key)
    }

    /// Получает все опции конфигурации в виде HashMap.
    ///
    /// # Возвращает
    ///
    /// HashMap, содержащий все текущие пары ключ-значение конфигурации.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # fn example(conn: &cloudpub_sdk::Connection) {
    /// let options = conn.options();
    /// for (key, value) in options {
    ///     println!("Config: {} = {}", key, value);
    /// }
    /// # }
    /// ```
    pub fn options(&self) -> HashMap<String, String> {
        self.config.read().get_all_options().into_iter().collect()
    }

    /// Выходит из системы, очищая токен аутентификации.
    ///
    /// Это удаляет сохраненный токен аутентификации как из памяти, так и
    /// из файла конфигурации. Соединение нужно будет повторно аутентифицировать
    /// для дальнейших операций.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # fn example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// conn.logout()?;
    /// println!("Выход выполнен успешно");
    /// # Ok(())
    /// # }
    /// ```
    pub fn logout(&self) -> Result<()> {
        self.config.write().token = None;
        self.config.write().save()?;
        Ok(())
    }

    /// Регистрирует сервис на сервере CloudPub.
    ///
    /// Этот метод регистрирует локальный сервис для доступа через CloudPub.
    /// Сервису будет присвоен уникальный GUID и URL для удаленного доступа.
    ///
    /// # Аргументы
    ///
    /// * `protocol` - Тип протокола (HTTP, HTTPS, TCP, UDP, WS, WSS, RTSP)
    /// * `address` - Локальный адрес сервиса. Для RTSP с авторизацией используйте формат: `rtsp://user:pass@host:port/path`
    /// * `name` - Опциональное человекочитаемое имя для сервиса
    /// * `auth` - Опциональный метод аутентификации для доступа к сервису
    /// * `acl` - Опциональный список контроля доступа для фильтрации IP
    /// * `headers` - Опциональные HTTP заголовки для добавления к ответам
    /// * `rules` - Опциональные правила фильтрации для фильтрации запросов
    ///
    /// # Возвращает
    ///
    /// Возвращает `ServerEndpoint`, содержащий:
    /// - `guid`: Уникальный идентификатор для сервиса
    /// - URL информацию для доступа к сервису
    /// - Статус сервиса и метаданные
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// use cloudpub_common::protocol::{Protocol, Auth, Endpoint, Acl, Header, FilterRule};
    ///
    /// // HTTP service
    /// let endpoint = conn.register(
    ///     Protocol::Http,
    ///     "localhost:3000".to_string(),
    ///     Some("My Web App".to_string()),
    ///     Some(Auth::Basic),
    ///     None,
    ///     None,
    ///     None,
    /// ).await?;
    ///
    /// // HTTP service with ACL and headers
    /// let acl = vec![Acl { user: "admin".to_string(), role: cloudpub_common::protocol::Role::Admin as i32 }];
    /// let headers = vec![Header { name: "X-Custom".to_string(), value: "test".to_string() }];
    /// let endpoint = conn.register(
    ///     Protocol::Http,
    ///     "localhost:8080".to_string(),
    ///     Some("API Server".to_string()),
    ///     Some(Auth::None),
    ///     Some(acl),
    ///     Some(headers),
    ///     None,
    /// ).await?;
    ///
    /// // RTSP service with credentials in URL
    /// let rtsp_endpoint = conn.register(
    ///     Protocol::Rtsp,
    ///     "rtsp://camera:secret@localhost:554/stream".to_string(),
    ///     Some("Security Camera".to_string()),
    ///     Some(Auth::None),
    ///     None,
    ///     None,
    ///     None,
    /// ).await?;
    ///
    /// println!("Сервис зарегистрирован по адресу: {}", endpoint.as_url());
    /// println!("GUID сервиса: {}", endpoint.guid);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register(
        &mut self,
        protocol: cloudpub_common::protocol::Protocol,
        address: String,
        name: Option<String>,
        auth: Option<cloudpub_common::protocol::Auth>,
        acl: Option<Vec<cloudpub_common::protocol::Acl>>,
        headers: Option<Vec<cloudpub_common::protocol::Header>>,
        rules: Option<Vec<cloudpub_common::protocol::FilterRule>>,
    ) -> Result<cloudpub_common::protocol::ServerEndpoint> {
        let publish_args = PublishArgs {
            protocol,
            address,
            username: None,
            password: None,
            name,
            auth,
            acl: acl.unwrap_or_default(),
            headers: headers.unwrap_or_default(),
            rules: rules.unwrap_or_default(),
        };

        let endpoint_start = publish_args.parse()?;

        self.command_tx
            .send(Message::EndpointStart(endpoint_start))
            .await?;

        // Wait for response
        let event = self
            .wait_for_event(|event| matches!(event, ConnectionEvent::Endpoint(_)))
            .await?;

        if let ConnectionEvent::Endpoint(endpoint) = event {
            Ok(*endpoint)
        } else {
            anyhow::bail!("Unexpected state")
        }
    }

    /// Публикует сервис на сервере CloudPub.
    ///
    /// Это псевдоним для `register()`, который запускает сервис сразу после регистрации
    ///
    /// # Аргументы
    ///
    /// * `protocol` - Тип протокола (HTTP, HTTPS, TCP, UDP, WS, WSS, RTSP)
    /// * `address` - Локальный адрес сервиса. Для RTSP с авторизацией используйте формат: `rtsp://user:pass@host:port/path`
    /// * `name` - Опциональное человекочитаемое имя для сервиса
    /// * `auth` - Опциональный метод аутентификации для доступа к сервису
    /// * `acl` - Опциональный список контроля доступа для фильтрации IP
    /// * `headers` - Опциональные HTTP заголовки для добавления к ответам
    /// * `rules` - Опциональные правила фильтрации для фильтрации запросов
    ///
    /// # Возвращает
    ///
    /// Возвращает `ServerEndpoint` с деталями сервиса и URL доступа.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// use cloudpub_common::protocol::{Protocol, Auth, Endpoint, Acl, FilterRule};
    ///
    /// // Publish a TCP service
    /// let endpoint = conn.publish(
    ///     Protocol::Tcp,
    ///     "localhost:8080".to_string(),
    ///     Some("TCP Server".to_string()),
    ///     Some(Auth::None),
    ///     None,
    ///     None,
    ///     None,
    /// ).await?;
    ///
    /// // Publish HTTP service with ACL
    /// let acl = vec![Acl { user: "reader".to_string(), role: cloudpub_common::protocol::Role::Reader as i32 }];
    /// let endpoint = conn.publish(
    ///     Protocol::Http,
    ///     "localhost:3000".to_string(),
    ///     Some("Internal API".to_string()),
    ///     Some(Auth::Basic),
    ///     Some(acl),
    ///     None,
    ///     None,
    /// ).await?;
    ///
    /// // Publish RTSP with embedded credentials
    /// let rtsp = conn.publish(
    ///     Protocol::Rtsp,
    ///     "rtsp://admin:password@192.168.1.100:554/live/ch0".to_string(),
    ///     Some("IP Camera".to_string()),
    ///     Some(Auth::Basic),
    ///     None,
    ///     None,
    ///     None,
    /// ).await?;
    ///
    /// println!("TCP сервис доступен по адресу: {}", endpoint.as_url());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn publish(
        &mut self,
        protocol: cloudpub_common::protocol::Protocol,
        address: String,
        name: Option<String>,
        auth: Option<cloudpub_common::protocol::Auth>,
        acl: Option<Vec<cloudpub_common::protocol::Acl>>,
        headers: Option<Vec<cloudpub_common::protocol::Header>>,
        rules: Option<Vec<cloudpub_common::protocol::FilterRule>>,
    ) -> Result<cloudpub_common::protocol::ServerEndpoint> {
        // Same as register for now
        let endpoint = self
            .register(protocol, address, name, auth, acl, headers, rules)
            .await?;
        self.start(endpoint.guid.clone()).await?;
        Ok(endpoint)
    }

    /// Выводит список всех зарегистрированных сервисов.
    ///
    /// Возвращает список всех сервисов, в данный момент зарегистрированных на сервере,
    /// включая их статус, URL и метаданные.
    ///
    /// # Возвращает
    ///
    /// Вектор структур `ServerEndpoint`, содержащих информацию о сервисах.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// use cloudpub_common::protocol::Endpoint;
    ///
    /// let services = conn.ls().await?;
    ///
    /// for service in services {
    ///     let name = service.client.as_ref()
    ///         .and_then(|c| c.description.clone())
    ///         .unwrap_or_else(|| "Безымянный".to_string());
    ///     println!("Сервис: {} ({})" ,
    ///         name,
    ///         service.guid
    ///     );
    ///     println!("  URL: {}", service.as_url());
    ///     println!("  Статус: {}", service.status.unwrap_or_else(|| "Неизвестен".to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ls(&mut self) -> Result<Vec<cloudpub_common::protocol::ServerEndpoint>> {
        self.command_tx
            .send(Message::EndpointList(EndpointList {}))
            .await?;
        let event = self
            .wait_for_event(|event| matches!(event, ConnectionEvent::List(_)))
            .await?;

        if let ConnectionEvent::List(list) = event {
            Ok(list)
        } else {
            anyhow::bail!("Unexpected state")
        }
    }

    /// Запускает сервис по его GUID.
    ///
    /// Запускает ранее зарегистрированный сервис, который мог быть остановлен.
    /// Это делает сервис снова доступным через его CloudPub URL.
    ///
    /// # Аргументы
    ///
    /// * `guid` - Уникальный идентификатор сервиса для запуска
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// let services = conn.ls().await?;
    /// if let Some(service) = services.first() {
    ///     conn.start(service.guid.clone()).await?;
    ///     println!("Запущен сервис: {}", service.guid);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&mut self, guid: String) -> Result<()> {
        self.command_tx
            .send(Message::EndpointGuidStart(EndpointStart { guid }))
            .await?;
        // Wait for response
        self.wait_for_event(|event| matches!(event, ConnectionEvent::Endpoint(_)))
            .await?;
        Ok(())
    }

    /// Останавливает сервис по его GUID.
    ///
    /// Временно останавливает сервис, делая его недоступным через CloudPub.
    /// Регистрация сервиса сохраняется и может быть перезапущена позже.
    ///
    /// # Arguments
    ///
    /// * `guid` - Уникальный идентификатор сервиса для остановки
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// conn.stop("service-guid-123".to_string()).await?;
    /// println!("Сервис остановлен");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stop(&mut self, guid: String) -> Result<()> {
        self.command_tx
            .send(Message::EndpointStop(EndpointStop { guid }))
            .await?;
        self.wait_for_event(|event| matches!(event, ConnectionEvent::Acknowledged))
            .await?;
        Ok(())
    }

    /// Отменяет публикацию (удаляет) сервис по его GUID.
    ///
    /// Навсегда удаляет регистрацию сервиса с сервера.
    /// Сервис больше не будет доступен через CloudPub.
    ///
    /// # Arguments
    ///
    /// * `guid` - Уникальный идентификатор сервиса для отмены публикации
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// conn.unpublish("service-guid-123".to_string()).await?;
    /// println!("Публикация сервиса отменена и удалена");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unpublish(&mut self, guid: String) -> Result<()> {
        self.command_tx
            .send(Message::EndpointRemove(EndpointRemove { guid }))
            .await?;
        self.wait_for_event(|event| matches!(event, ConnectionEvent::Acknowledged))
            .await?;
        Ok(())
    }

    /// Удаляет все зарегистрированные сервисы.
    ///
    /// Этот метод удаляет все сервисы, зарегистрированные текущим пользователем.
    /// Используйте с осторожностью, так как эта операция не может быть отменена.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// conn.clean().await?;
    /// println!("Все сервисы удалены");
    ///
    /// let services = conn.ls().await?;
    /// assert_eq!(services.len(), 0);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn clean(&mut self) -> Result<()> {
        self.command_tx
            .send(Message::EndpointClear(EndpointClear {}))
            .await?;
        self.wait_for_event(|event| matches!(event, ConnectionEvent::Acknowledged))
            .await?;
        Ok(())
    }

    /// Пингует сервер для измерения задержки.
    ///
    /// Создает временную конечную точку пинга и измеряет время прохождения
    /// туда и обратно до сервера. Полезно для проверки работоспособности соединения и задержки.
    ///
    /// # Возвращает
    ///
    /// Задержку пинга в микросекундах.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example(conn: &mut cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// let latency_us = conn.ping().await?;
    /// println!("Задержка сервера: {}μs ({:.2}мс)", latency_us, latency_us as f64 / 1000.0);
    ///
    /// if latency_us > 100_000 {  // 100ms
    ///     println!("Предупреждение: Обнаружена высокая задержка");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ping(&mut self) -> Result<u64> {
        ping::publish(self.command_tx.clone()).await?;
        let event = self
            .wait_for_event(|event| matches!(event, ConnectionEvent::Endpoint(_)))
            .await?;
        let endpoint = if let ConnectionEvent::Endpoint(ep) = event {
            *ep
        } else {
            anyhow::bail!("Unexpected state")
        };
        Ok(ping::ping_test(endpoint, true).await?.parse::<u64>()?)
    }

    /// Очищает локальный каталог кэша.
    ///
    /// Удаляет все кэшированные данные, включая временные файлы и логи.
    /// Это может быть полезно для устранения неполадок или освобождения дискового пространства.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # fn example(conn: &cloudpub_sdk::Connection) -> anyhow::Result<()> {
    /// conn.purge()?;
    /// println!("Кэш успешно очищен");
    /// # Ok(())
    /// # }
    /// ```
    pub fn purge(&self) -> Result<()> {
        let cache_dir = get_cache_dir("")?;
        std::fs::remove_dir_all(&cache_dir).ok();
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Send Break message to stop the client gracefully
        self.command_tx
            .try_send(Message::Break(Break {
                guid: String::new(),
            }))
            .ok();

        // Cancel the receiver task
        if let Some(handle) = self.receiver_handle.take() {
            handle.abort();
        }

        // Abort the client task if it's still running
        if let Some(handle) = self.client_handle.take() {
            handle.abort();
        }
    }
}
