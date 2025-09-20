//! # CloudPub Python SDK
//!
//! Python-привязки для платформы сервисов CloudPub, обеспечивающие безопасное туннелирование и публикацию сервисов.
//!
//! ## Документация
//!
//! Для получения полной документации, примеров и справочника по API, пожалуйста, посетите
//! [документацию Python SDK](../docs/index.rst) или соберите документацию локально:
//!
//! ```bash
//! cd sdk/python/docs
//! make html
//! ```
//!
//! ## Быстрый пример
//!
//! ```python
//! from cloudpub_python_sdk import Connection, Protocol, Auth
//!
//! # Подключение и публикация сервиса
//! conn = Connection(email="user@example.com", password="password")
//! endpoint = conn.publish(
//!     Protocol.HTTP,
//!     "localhost:3000",
//!     name="Моё веб-приложение",
//!     auth=Auth.NONE
//! )
//! print(f"Сервис опубликован по адресу: {endpoint.url}")
//! ```
//!
//! ## Лицензия
//!
//! Лицензируется под лицензией Apache License, версия 2.0.

#![allow(clippy::upper_case_acronyms)] // Python enums используют заглавные буквы по соглашению
#![allow(clippy::await_holding_lock)] // Ложные срабатывания с parking_lot в block_on
#![allow(clippy::useless_conversion)] // Макросы PyO3 требуют эти преобразования

use parking_lot::RwLock;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;

use cloudpub_common::protocol;
use cloudpub_common::protocol::Endpoint;
use cloudpub_sdk::Connection as RustConnection;
use pyo3::create_exception;

// Создание пользовательских исключений для ошибок CloudPub
// Подавление предупреждений от внутренних макросов PyO3 о устаревшей функции gil-refs
#[allow(unexpected_cfgs)]
mod exceptions {
    use super::*;
    create_exception!(cloudpub_python_sdk, CloudPubError, PyException);
    create_exception!(cloudpub_python_sdk, ConnectionError, CloudPubError);
    create_exception!(cloudpub_python_sdk, AuthenticationError, CloudPubError);
    create_exception!(cloudpub_python_sdk, ConfigError, CloudPubError);
}

use exceptions::{AuthenticationError, CloudPubError, ConfigError};

/// Перечисление протоколов для публикации сервисов
///
/// Определяет тип протокола, который будет использоваться для доступа к публикуемому сервису.
/// Каждый протокол имеет свои особенности конфигурации и использования.
///
/// Примеры:
///     >>> from cloudpub_python_sdk import Protocol
///     >>> protocol = Protocol.HTTP  # Для HTTP-сервисов
///     >>> protocol = Protocol.TCP   # Для TCP-сокетов
#[pyclass(eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
enum Protocol {
    HTTP,
    HTTPS,
    TCP,
    UDP,
    ONEC,
    MINECRAFT,
    WEBDAV,
    RTSP,
}

#[pymethods]
impl Protocol {
    fn __str__(&self) -> &str {
        match self {
            Protocol::HTTP => "http",
            Protocol::HTTPS => "https",
            Protocol::TCP => "tcp",
            Protocol::UDP => "udp",
            Protocol::ONEC => "1c",
            Protocol::MINECRAFT => "minecraft",
            Protocol::WEBDAV => "webdav",
            Protocol::RTSP => "rtsp",
        }
    }

    fn __repr__(&self) -> String {
        format!("Protocol.{:?}", self)
    }
}

impl From<Protocol> for protocol::Protocol {
    fn from(p: Protocol) -> Self {
        match p {
            Protocol::HTTP => protocol::Protocol::Http,
            Protocol::HTTPS => protocol::Protocol::Https,
            Protocol::TCP => protocol::Protocol::Tcp,
            Protocol::UDP => protocol::Protocol::Udp,
            Protocol::ONEC => protocol::Protocol::OneC,
            Protocol::MINECRAFT => protocol::Protocol::Minecraft,
            Protocol::WEBDAV => protocol::Protocol::Webdav,
            Protocol::RTSP => protocol::Protocol::Rtsp,
        }
    }
}

/// Перечисление типов аутентификации для защиты публикуемых сервисов
///
/// Определяет метод аутентификации, который будет использоваться для доступа к сервису.
///
/// Варианты:
///     NONE: Без аутентификации, открытый доступ
///     BASIC: HTTP Basic аутентификация (логин/пароль)
///     FORM: Аутентификация через веб-форму
///
/// Примеры:
///     >>> from cloudpub_python_sdk import Auth
///     >>> auth = Auth.BASIC  # Использовать Basic аутентификацию
///     >>> auth = Auth.NONE   # Открытый доступ без аутентификации
#[pyclass(eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
enum Auth {
    NONE,
    BASIC,
    FORM,
}

#[pymethods]
impl Auth {
    fn __str__(&self) -> &str {
        match self {
            Auth::NONE => "none",
            Auth::BASIC => "basic",
            Auth::FORM => "form",
        }
    }

    fn __repr__(&self) -> String {
        format!("Auth.{:?}", self)
    }
}

impl From<Auth> for protocol::Auth {
    fn from(a: Auth) -> Self {
        match a {
            Auth::NONE => protocol::Auth::None,
            Auth::BASIC => protocol::Auth::Basic,
            Auth::FORM => protocol::Auth::Form,
        }
    }
}

/// Перечисление ролей для Python
#[pyclass(eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
enum Role {
    NOBODY,
    ADMIN,
    READER,
    WRITER,
}

#[pymethods]
impl Role {
    fn __str__(&self) -> &str {
        match self {
            Role::NOBODY => "nobody",
            Role::ADMIN => "admin",
            Role::READER => "reader",
            Role::WRITER => "writer",
        }
    }

    fn __repr__(&self) -> String {
        format!("Role.{:?}", self)
    }
}

impl From<Role> for i32 {
    fn from(r: Role) -> Self {
        match r {
            Role::NOBODY => protocol::Role::Nobody as i32,
            Role::ADMIN => protocol::Role::Admin as i32,
            Role::READER => protocol::Role::Reader as i32,
            Role::WRITER => protocol::Role::Writer as i32,
        }
    }
}

/// Запись списка контроля доступа (ACL) для управления правами пользователей
///
/// Определяет права доступа конкретного пользователя к публикуемому сервису.
/// Используется для тонкой настройки безопасности на уровне пользователей.
///
/// Атрибуты:
///     user (str): Email или идентификатор пользователя
///     role (Role): Роль пользователя, определяющая уровень доступа
///
/// Примеры:
///     >>> from cloudpub_python_sdk import Acl, Role
///     >>> acl = Acl(user="admin@example.com", role=Role.ADMIN)
///     >>> acl = Acl(user="viewer@example.com", role=Role.READER)
#[pyclass]
#[derive(Clone, Debug)]
struct Acl {
    #[pyo3(get, set)]
    user: String,
    #[pyo3(get, set)]
    role: Role,
}

#[pymethods]
impl Acl {
    #[new]
    fn new(user: String, role: Role) -> Self {
        Acl { user, role }
    }

    fn __str__(&self) -> String {
        format!("{}: {}", self.user, self.role.__str__())
    }

    fn __repr__(&self) -> String {
        format!("Acl(user={:?}, role={})", self.user, self.role.__repr__())
    }
}

impl From<Acl> for protocol::Acl {
    fn from(a: Acl) -> Self {
        protocol::Acl {
            user: a.user,
            role: a.role.into(),
        }
    }
}

/// HTTP-заголовок для добавления в ответы сервера
///
/// Позволяет добавлять пользовательские HTTP-заголовки к ответам публикуемого сервиса.
/// Полезно для настройки CORS, кэширования, безопасности и других HTTP-параметров.
///
/// Атрибуты:
///     name (str): Имя заголовка (например, "X-Custom-Header")
///     value (str): Значение заголовка
///
/// Примеры:
///     >>> from cloudpub_python_sdk import Header
///     >>> header = Header(name="X-Custom-Id", value="12345")
///     >>> cors_header = Header(name="Access-Control-Allow-Origin", value="*")
#[pyclass]
#[derive(Clone, Debug)]
struct Header {
    #[pyo3(get, set)]
    name: String,
    #[pyo3(get, set)]
    value: String,
}

#[pymethods]
impl Header {
    #[new]
    fn new(name: String, value: String) -> Self {
        Header { name, value }
    }

    fn __str__(&self) -> String {
        format!("{}: {}", self.name, self.value)
    }

    fn __repr__(&self) -> String {
        format!("Header(name={:?}, value={:?})", self.name, self.value)
    }
}

impl From<Header> for protocol::Header {
    fn from(h: Header) -> Self {
        protocol::Header {
            name: h.name,
            value: h.value,
        }
    }
}

/// Перечисление действий фильтра для Python
#[pyclass(eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
enum FilterAction {
    ALLOW,
    DENY,
    REDIRECT,
    LOG,
}

#[pymethods]
impl FilterAction {
    fn __str__(&self) -> &str {
        match self {
            FilterAction::ALLOW => "allow",
            FilterAction::DENY => "deny",
            FilterAction::REDIRECT => "redirect",
            FilterAction::LOG => "log",
        }
    }

    fn __repr__(&self) -> String {
        format!("FilterAction.{:?}", self)
    }
}

impl From<FilterAction> for i32 {
    fn from(a: FilterAction) -> Self {
        match a {
            FilterAction::ALLOW => protocol::FilterAction::FilterAllow as i32,
            FilterAction::DENY => protocol::FilterAction::FilterDeny as i32,
            FilterAction::REDIRECT => protocol::FilterAction::FilterRedirect as i32,
            FilterAction::LOG => protocol::FilterAction::FilterLog as i32,
        }
    }
}

/// Правило фильтрации для управления доступом к сервису
///
/// Определяет правила обработки входящих запросов на основе различных критериев.
/// Правила применяются в порядке, определённом полем order. Поддерживает сложные
/// выражения с использованием переменных, операторов сравнения и логических операторов.
///
/// Атрибуты:
///     data (str): Выражение фильтра с использованием переменных и операторов
///     action_type (FilterAction): Действие при совпадении правила
///     action_value (str, optional): Дополнительное значение для действия (например, URL для редиректа)
///     order (int): Порядок применения правила (по умолчанию 0)
///
/// Доступные переменные:
///     - ip.src, ip.dst: IP-адреса источника и назначения
///     - port.src, port.dst: Порты источника и назначения
///     - protocol: Протокол ("tcp", "udp", "http")
///     - http.host, http.path, http.method: HTTP-поля
///     - http.headers["Name"]: HTTP-заголовки
///     - geo.country, geo.region, geo.city, geo.code: Геолокация
///
/// Примеры:
///     >>> from cloudpub_python_sdk import FilterRule, FilterAction
///     >>> # Блокировать конкретный IP-адрес
///     >>> rule1 = FilterRule(data='ip.src == 192.168.1.100', action_type=FilterAction.DENY)
///     >>> # Разрешить доступ только из локальной сети
///     >>> rule2 = FilterRule(data='ip.src >= 192.168.1.0 and ip.src <= 192.168.1.255',
///     ...                    action_type=FilterAction.ALLOW)
///     >>> # Заблокировать доступ к админке
///     >>> rule3 = FilterRule(data='http.path matches "^/admin.*"', action_type=FilterAction.DENY)
///     >>> # Перенаправить старый путь на новый
///     >>> rule4 = FilterRule(data='http.path == "/old-api"', action_type=FilterAction.REDIRECT,
///     ...                    action_value="/api/v2", order=1)
///     >>> # Разрешить только GET-запросы к API
///     >>> rule5 = FilterRule(data='http.method == "GET" and http.path starts_with "/api/"',
///     ...                    action_type=FilterAction.ALLOW)
///     >>> # Геофильтрация - разрешить только из определённых стран
///     >>> rule6 = FilterRule(data='geo.code == "US" or geo.code == "CA"',
///     ...                    action_type=FilterAction.ALLOW)
#[pyclass]
#[derive(Clone, Debug)]
struct FilterRule {
    #[pyo3(get, set)]
    order: i32,
    #[pyo3(get, set)]
    action_value: Option<String>,
    #[pyo3(get, set)]
    action_type: FilterAction,
    #[pyo3(get, set)]
    data: String,
}

#[pymethods]
impl FilterRule {
    #[new]
    #[pyo3(signature = (data, action_type, action_value=None, order=0))]
    fn new(
        data: String,
        action_type: FilterAction,
        action_value: Option<String>,
        order: i32,
    ) -> Self {
        FilterRule {
            order,
            action_value,
            action_type,
            data,
        }
    }

    fn __str__(&self) -> String {
        if let Some(ref value) = self.action_value {
            format!("{}: {} -> {}", self.data, self.action_type.__str__(), value)
        } else {
            format!("{}: {}", self.data, self.action_type.__str__())
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FilterRule(data={:?}, action_type={}, action_value={:?}, order={})",
            self.data,
            self.action_type.__repr__(),
            self.action_value,
            self.order
        )
    }
}

impl From<FilterRule> for protocol::FilterRule {
    fn from(r: FilterRule) -> Self {
        protocol::FilterRule {
            order: r.order,
            action_value: r.action_value,
            action_type: r.action_type.into(),
            data: r.data,
        }
    }
}

/// Представление ServerEndpoint для Python
#[pyclass]
#[derive(Clone)]
struct ServerEndpoint {
    #[pyo3(get)]
    guid: String,
    #[pyo3(get)]
    status: Option<String>,
    #[pyo3(get)]
    remote_proto: String,
    #[pyo3(get)]
    remote_addr: String,
    #[pyo3(get)]
    remote_port: u32,
    #[pyo3(get)]
    url: String,
    #[pyo3(get)]
    error: String,
}

#[pymethods]
impl ServerEndpoint {
    fn __repr__(&self) -> String {
        format!(
            "ServerEndpoint(guid={}, status={:?}, proto={}, addr={}:{})",
            self.guid, self.status, self.remote_proto, self.remote_addr, self.remote_port
        )
    }

    fn __str__(&self) -> String {
        self.url.clone()
    }
}

/// Python-обёртка для клиента CloudPub
#[pyclass]
struct Connection {
    connection: Arc<RwLock<RustConnection>>,
    runtime: tokio::runtime::Runtime,
}

#[pymethods]
impl Connection {
    /// Инициализация нового клиента CloudPub с аутентификацией
    ///
    /// Следует указать либо токен, либо email и пароль для аутентификации.
    ///
    /// Аргументы:
    ///     config_path (str, опционально): Путь к файлу конфигурации
    ///     log_level (str, опционально): Уровень логирования (по умолчанию: "debug")
    ///     verbose (bool, опционально): Вывод лога в stderr (по умолчанию: False)
    ///     token (str, опционально): Токен аутентификации
    ///     email (str, опционально): Email для входа
    ///     password (str, опционально): Пароль для входа
    ///
    /// Исключения:
    ///     AuthenticationError: Если аутентификация не удалась или не предоставлены учетные данные
    #[new]
    #[pyo3(signature = (config_path=None, log_level="debug".to_string(), verbose=false, token=None, email=None, password=None))]
    fn new(
        config_path: Option<String>,
        log_level: String,
        verbose: bool,
        token: Option<String>,
        email: Option<String>,
        password: Option<String>,
    ) -> PyResult<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| PyException::new_err(format!("Не удалось создать runtime: {}", e)))?;

        // Создание соединения с путём конфигурации и аутентификацией
        let path = config_path.as_deref().map(std::path::Path::new);

        // Использование паттерна builder для создания соединения
        let mut builder = RustConnection::builder()
            .log_level(log_level)
            .verbose(verbose)
            .timeout_secs(30);

        if let Some(path) = path {
            builder = builder.config_path(path);
        }

        if let Some(token_value) = token {
            builder = builder.token(token_value);
        } else if let (Some(email_value), Some(password_value)) = (email, password) {
            builder = builder.credentials(email_value, password_value);
        }

        // Добавление колбэка проверки сигналов для Python
        let check_signal_fn = Arc::new(move || -> anyhow::Result<()> {
            Python::with_gil(|py| {
                py.check_signals()
                    .map_err(|e| anyhow::anyhow!("Прерывание сигнала Python: {}", e))
            })
        });
        builder = builder.check_signal_fn(check_signal_fn);

        // Сборка соединения в контексте runtime
        let connection = runtime
            .block_on(async { builder.build().await })
            .map_err(|e| PyException::new_err(format!("Не удалось создать соединение: {}", e)))?;
        let connection = Arc::new(RwLock::new(connection));

        let client = Connection {
            connection: connection.clone(),
            runtime,
        };

        // Проверка соединения вызовом ls для подтверждения работы аутентификации
        client
            .runtime
            .block_on(async {
                let mut conn = connection.write();
                conn.ls().await
            })
            .map_err(|e| {
                AuthenticationError::new_err(format!("Проверка аутентификации не удалась: {}", e))
            })?;

        Ok(client)
    }

    /// Установить значение конфигурации
    ///
    /// Аргументы:
    ///     key: Ключ конфигурации
    ///     value: Значение конфигурации
    ///
    /// Исключения:
    ///     ConfigError: Если установка конфигурации не удалась
    fn set(&self, key: String, value: String) -> PyResult<()> {
        self.connection.read().set(&key, &value).map_err(|e| {
            ConfigError::new_err(format!("Не удалось установить конфигурацию: {}", e))
        })?;
        Ok(())
    }

    /// Получить значение конфигурации
    ///
    /// Аргументы:
    ///     key: Ключ конфигурации
    ///
    /// Возвращает:
    ///     str: Значение конфигурации
    ///
    /// Исключения:
    ///     ConfigError: Если получение конфигурации не удалось
    fn get(&self, key: String) -> PyResult<String> {
        self.connection
            .read()
            .get(&key)
            .map_err(|e| ConfigError::new_err(format!("Не удалось получить конфигурацию: {}", e)))
    }

    /// Получить все опции конфигурации
    ///
    /// Возвращает:
    ///     dict: Словарь пар ключ-значение конфигурации
    fn options(&self, py: Python) -> PyResult<Py<PyDict>> {
        let options = self.connection.read().options();
        let dict = PyDict::new_bound(py);
        for (key, value) in options {
            dict.set_item(key, value)?;
        }
        Ok(dict.into())
    }

    /// Выйти и очистить сохранённый токен
    ///
    /// Исключения:
    ///     ConfigError: Если сохранение конфигурации не удалось
    fn logout(&self) -> PyResult<()> {
        self.connection
            .read()
            .logout()
            .map_err(|e| ConfigError::new_err(format!("Не удалось выйти: {}", e)))?;
        Ok(())
    }

    /// Зарегистрировать сервис на сервере
    ///
    /// Аргументы:
    ///     protocol (Protocol): Используемый протокол
    ///     address (str): URL, адрес сокета, порт или путь к файлу. Для RTSP с аутентификацией используйте формат: rtsp://user:pass@host:port/path
    ///     name (str, опционально): Имя сервиса
    ///     auth (Auth, опционально): Тип аутентификации
    ///     acl (list[Acl], опционально): Список контроля доступа для фильтрации IP
    ///     headers (list[Header], опционально): HTTP-заголовки для добавления в ответы
    ///     rules (list[FilterRule], опционально): Правила фильтрации для фильтрации запросов
    ///
    /// Возвращает:
    ///     ServerEndpoint: Зарегистрированная конечная точка
    ///
    /// Исключения:
    ///     CloudPubError: Если регистрация не удалась
    #[pyo3(signature = (protocol, address, name=None, auth=None, acl=None, headers=None, rules=None))]
    fn register(
        &self,
        protocol: Protocol,
        address: String,
        name: Option<String>,
        auth: Option<Auth>,
        acl: Option<Vec<Acl>>,
        headers: Option<Vec<Header>>,
        rules: Option<Vec<FilterRule>>,
    ) -> PyResult<ServerEndpoint> {
        let proto: protocol::Protocol = protocol.into();
        let auth_type = auth.map(|a| a.into());
        let acl_list = acl.map(|list| list.into_iter().map(|a| a.into()).collect());
        let header_list = headers.map(|list| list.into_iter().map(|h| h.into()).collect());
        let rule_list = rules.map(|list| list.into_iter().map(|r| r.into()).collect());

        let endpoint = self
            .runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.register(
                    proto,
                    address,
                    name,
                    auth_type,
                    acl_list,
                    header_list,
                    rule_list,
                )
                .await
            })
            .map_err(|e| CloudPubError::new_err(format!("Регистрация не удалась: {}", e)))?;

        Ok(ServerEndpoint {
            url: endpoint.as_url(),
            guid: endpoint.guid,
            status: endpoint.status,
            remote_proto: format!("{:?}", endpoint.remote_proto).to_lowercase(),
            remote_addr: endpoint.remote_addr,
            remote_port: endpoint.remote_port,
            error: endpoint.error,
        })
    }

    /// Опубликовать сервис (зарегистрировать и запустить)
    ///
    /// Аргументы:
    ///     protocol (Protocol): Используемый протокол
    ///     address (str): URL, адрес сокета, порт или путь к файлу. Для RTSP с аутентификацией используйте формат: rtsp://user:pass@host:port/path
    ///     name (str, опционально): Имя сервиса
    ///     auth (Auth, опционально): Тип аутентификации
    ///     acl (list[Acl], опционально): Список контроля доступа для фильтрации IP
    ///     headers (list[Header], опционально): HTTP-заголовки для добавления в ответы
    ///     rules (list[FilterRule], опционально): Правила фильтрации для фильтрации запросов
    ///
    /// Возвращает:
    ///     ServerEndpoint: Опубликованная конечная точка
    ///
    /// Исключения:
    ///     CloudPubError: Если публикация не удалась
    #[pyo3(signature = (protocol, address, name=None, auth=None, acl=None, headers=None, rules=None))]
    fn publish(
        &self,
        protocol: Protocol,
        address: String,
        name: Option<String>,
        auth: Option<Auth>,
        acl: Option<Vec<Acl>>,
        headers: Option<Vec<Header>>,
        rules: Option<Vec<FilterRule>>,
    ) -> PyResult<ServerEndpoint> {
        let proto: protocol::Protocol = protocol.into();
        let auth_type = auth.map(|a| a.into());
        let acl_list = acl.map(|list| list.into_iter().map(|a| a.into()).collect());
        let header_list = headers.map(|list| list.into_iter().map(|h| h.into()).collect());
        let rule_list = rules.map(|list| list.into_iter().map(|r| r.into()).collect());

        let endpoint = self
            .runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.publish(
                    proto,
                    address,
                    name,
                    auth_type,
                    acl_list,
                    header_list,
                    rule_list,
                )
                .await
            })
            .map_err(|e| CloudPubError::new_err(format!("Публикация не удалась: {}", e)))?;

        Ok(ServerEndpoint {
            url: endpoint.as_url(),
            guid: endpoint.guid,
            status: endpoint.status,
            remote_proto: format!("{:?}", endpoint.remote_proto).to_lowercase(),
            remote_addr: endpoint.remote_addr,
            remote_port: endpoint.remote_port,
            error: endpoint.error,
        })
    }

    /// Запустить публикацию по GUID
    ///
    /// Аргументы:
    ///     guid: GUID сервиса для запуска
    ///
    /// Исключения:
    ///     CloudPubError: Если запуск не удался
    fn start(&self, guid: String) -> PyResult<()> {
        self.runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.start(guid).await
            })
            .map_err(|e| CloudPubError::new_err(format!("Запуск не удался: {}", e)))?;
        Ok(())
    }

    /// Остановить публикацию по GUID
    ///
    /// Аргументы:
    ///     guid: GUID сервиса для остановки
    ///
    /// Исключения:
    ///     CloudPubError: Если остановка не удалась
    fn stop(&self, guid: String) -> PyResult<()> {
        self.runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.stop(guid).await
            })
            .map_err(|e| CloudPubError::new_err(format!("Остановка не удалась: {}", e)))?;
        Ok(())
    }

    /// Отменить публикацию (отменить регистрацию) сервиса по GUID
    ///
    /// Аргументы:
    ///     guid: GUID сервиса для отмены публикации
    ///
    /// Исключения:
    ///     CloudPubError: Если отмена публикации не удалась
    fn unpublish(&self, guid: String) -> PyResult<()> {
        self.runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.unpublish(guid).await
            })
            .map_err(|e| CloudPubError::new_err(format!("Отмена публикации не удалась: {}", e)))?;
        Ok(())
    }

    /// Получить список всех зарегистрированных сервисов
    ///
    /// Возвращает:
    ///     list[ServerEndpoint]: Список зарегистрированных конечных точек
    ///
    /// Исключения:
    ///     CloudPubError: Если получение списка не удалось
    fn ls(&self) -> PyResult<Vec<ServerEndpoint>> {
        let endpoints = self
            .runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.ls().await
            })
            .map_err(|e| CloudPubError::new_err(format!("Получение списка не удалось: {}", e)))?;

        // Преобразование конечных точек в объекты Python ServerEndpoint
        let py_endpoints: Vec<ServerEndpoint> = endpoints
            .into_iter()
            .map(|ep| ServerEndpoint {
                url: ep.as_url(),
                guid: ep.guid,
                status: ep.status,
                remote_proto: format!("{:?}", ep.remote_proto).to_lowercase(),
                remote_addr: ep.remote_addr,
                remote_port: ep.remote_port,
                error: ep.error,
            })
            .collect();

        Ok(py_endpoints)
    }

    /// Очистить (удалить) все зарегистрированные сервисы
    ///
    /// Исключения:
    ///     CloudPubError: Если очистка не удалась
    fn clean(&self) -> PyResult<()> {
        self.runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.clean().await
            })
            .map_err(|e| CloudPubError::new_err(format!("Очистка не удалась: {}", e)))?;
        Ok(())
    }

    /// Пропинговать сервер и измерить время отклика
    ///
    /// Возвращает:
    ///     int: Задержка пинга в микросекундах
    ///
    /// Исключения:
    ///     CloudPubError: Если пинг не удался
    fn ping(&self) -> PyResult<u64> {
        let result = self
            .runtime
            .block_on(async {
                let mut conn = self.connection.write();
                conn.ping().await
            })
            .map_err(|e| CloudPubError::new_err(format!("Пинг не удался: {}", e)))?;
        Ok(result)
    }

    /// Очистить директорию кэша
    ///
    /// Исключения:
    ///     CloudPubError: Если очистка не удалась
    fn purge(&self) -> PyResult<()> {
        self.connection
            .read()
            .purge()
            .map_err(|e| CloudPubError::new_err(format!("Очистка не удалась: {}", e)))?;
        Ok(())
    }
}

/// Python-модуль CloudPub
#[pymodule]
#[pyo3(name = "cloudpub_python_sdk")]
fn cloudpub_python_sdk(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Connection>()?;
    m.add_class::<Protocol>()?;
    m.add_class::<Auth>()?;
    m.add_class::<Role>()?;
    m.add_class::<FilterAction>()?;
    m.add_class::<Acl>()?;
    m.add_class::<Header>()?;
    m.add_class::<FilterRule>()?;
    m.add_class::<ServerEndpoint>()?;

    // Добавление исключений
    m.add(
        "CloudPubError",
        m.py().get_type_bound::<exceptions::CloudPubError>(),
    )?;
    m.add(
        "ConnectionError",
        m.py().get_type_bound::<exceptions::ConnectionError>(),
    )?;
    m.add(
        "AuthenticationError",
        m.py().get_type_bound::<exceptions::AuthenticationError>(),
    )?;
    m.add(
        "ConfigError",
        m.py().get_type_bound::<exceptions::ConfigError>(),
    )?;

    Ok(())
}
