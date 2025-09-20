use anyhow::{bail, Context, Result};
use cloudpub_client::client::run_client;
pub use cloudpub_client::config::{ClientConfig, ClientOpts};
use cloudpub_common::config::MaskedString;
use cloudpub_common::logging::init_log;
use dirs::cache_dir;
use futures::future::FutureExt;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;

use crate::connection::{CheckSignalFn, Connection, ConnectionEvent};

/// Строитель для создания и настройки экземпляров `Connection`.
///
/// `ConnectionBuilder` предоставляет удобный интерфейс для настройки
/// параметров соединения перед установлением соединения с сервером CloudPub.
///
/// # Пример
///
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// use cloudpub_sdk::Connection;
/// use std::time::Duration;
///
/// let conn = Connection::builder()
///     .config_path("/custom/config.toml")  // Пользовательский файл конфигурации
///     .log_level("debug")                  // Установка уровня логирования
///     .verbose(true)                       // Включить вывод в консоль
///     .credentials("user@example.com", "password")  // Учетные данные для аутентификации
///     .timeout(Duration::from_secs(30))    // Таймаут операций
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// # Методы аутентификации
///
/// Строитель поддерживает два метода аутентификации:
///
/// 1. **На основе токена**: Используйте `token()` для аутентификации с существующим токеном
/// 2. **Учетные данные**: Используйте `credentials()` или `email()`/`password()` для аутентификации по имени/паролю
///
/// # Значения по умолчанию
///
/// - Уровень логирования: "info"
/// - Подробный вывод: false
/// - Таймаут: 10 секунд
/// - Путь к конфигурации: Системное расположение по умолчанию (~/.config/cloudpub/client.toml)
pub struct ConnectionBuilder {
    config_path: Option<PathBuf>,
    log_level: String,
    verbose: bool,
    token: Option<String>,
    email: Option<String>,
    password: Option<String>,
    timeout: Duration,
    check_signal_fn: Option<CheckSignalFn>,
}

impl Default for ConnectionBuilder {
    fn default() -> Self {
        Self {
            config_path: None,
            log_level: "info".to_string(),
            verbose: false,
            token: None,
            email: None,
            password: None,
            timeout: Duration::from_secs(10),
            check_signal_fn: None,
        }
    }
}

impl ConnectionBuilder {
    /// Создает новый builder с настройками по умолчанию.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// use cloudpub_sdk::ConnectionBuilder;
    ///
    /// let builder = ConnectionBuilder::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Устанавливает путь к файлу конфигурации.
    ///
    /// По умолчанию SDK ищет файл конфигурации в стандартном системном расположении.
    /// Используйте этот метод для указания пользовательского расположения.
    ///
    /// # Аргументы
    ///
    /// * `path` - Путь к файлу конфигурации
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    /// use std::path::Path;
    ///
    /// let conn = Connection::builder()
    ///     .config_path(Path::new("/etc/cloudpub/config.toml"))
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn config_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.config_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Устанавливает уровень логирования для SDK.
    ///
    /// # Аргументы
    ///
    /// * `level` - Уровень логирования: "trace", "debug", "info", "warn", "error"
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// // Включить отладочное логирование
    /// let conn = Connection::builder()
    ///     .log_level("debug")
    ///     .verbose(true)  // Также выводить в консоль
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn log_level<S: Into<String>>(mut self, level: S) -> Self {
        self.log_level = level.into();
        self
    }

    /// Включает или отключает подробное логирование в консоль.
    ///
    /// При включении сообщения логов выводятся в stderr в дополнение к файлу логов.
    /// Это полезно для отладки и разработки.
    ///
    /// # Аргументы
    ///
    /// * `verbose` - true для включения вывода в консоль, false для отключения
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let conn = Connection::builder()
    ///     .verbose(true)  // Включить вывод в консоль
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Устанавливает токен аутентификации.
    ///
    /// Используйте этот метод для аутентификации на основе токена. Это взаимоисключающе
    /// с аутентификацией на основе учетных данных.
    ///
    /// # Аргументы
    ///
    /// * `token` - Токен аутентификации, полученный при предыдущем входе
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let conn = Connection::builder()
    ///     .token("your-auth-token-here")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn token<S: Into<String>>(mut self, token: S) -> Self {
        self.token = Some(token.into());
        self.email = None;
        self.password = None;
        self
    }

    /// Устанавливает email и пароль для аутентификации.
    ///
    /// Используйте этот метод для аутентификации на основе учетных данных. Это взаимоисключающе
    /// с аутентификацией на основе токена.
    ///
    /// # Аргументы
    ///
    /// * `email` - Email адрес пользователя
    /// * `password` - Пароль пользователя
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let conn = Connection::builder()
    ///     .credentials("user@example.com", "secure-password")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn credentials<S: Into<String>>(mut self, email: S, password: S) -> Self {
        self.email = Some(email.into());
        self.password = Some(password.into());
        self.token = None;
        self
    }

    /// Устанавливает только email адрес.
    ///
    /// Должен использоваться в сочетании с `password()`. Это полезно, когда
    /// учетные данные получаются отдельно.
    ///
    /// # Аргументы
    ///
    /// * `email` - Email адрес пользователя
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let conn = Connection::builder()
    ///     .email("user@example.com")
    ///     .password("secure-password")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn email<S: Into<String>>(mut self, email: S) -> Self {
        self.email = Some(email.into());
        self.token = None;
        self
    }

    /// Устанавливает только пароль.
    ///
    /// Должен использоваться в сочетании с `email()`. Это полезно, когда
    /// учетные данные получаются отдельно.
    ///
    /// # Аргументы
    ///
    /// * `password` - Пароль пользователя
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let email = std::env::var("CLOUDPUB_EMAIL")?;
    /// let password = std::env::var("CLOUDPUB_PASSWORD")?;
    ///
    /// let conn = Connection::builder()
    ///     .email(email)
    ///     .password(password)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn password<S: Into<String>>(mut self, password: S) -> Self {
        self.password = Some(password.into());
        self.token = None;
        self
    }

    /// Устанавливает таймаут для операций.
    ///
    /// Этот таймаут применяется ко всем асинхронным операциям, таким как register, publish, ls и т.д.
    /// По умолчанию 10 секунд.
    ///
    /// # Аргументы
    ///
    /// * `timeout` - Продолжительность таймаута операции
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    /// use std::time::Duration;
    ///
    /// let conn = Connection::builder()
    ///     .timeout(Duration::from_secs(60))  // Таймаут 1 минута
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Устанавливает таймаут в секундах.
    ///
    /// Удобный метод для установки таймаута в секундах вместо Duration.
    ///
    /// # Аргументы
    ///
    /// * `secs` - Таймаут в секундах
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// let conn = Connection::builder()
    ///     .timeout_secs(30)  // Таймаут 30 секунд
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// Устанавливает функцию для проверки сигналов прерывания.
    ///
    /// Это в основном используется языковыми обертками (например, Python) для проверки
    /// сигналов, таких как Ctrl+C, во время долговременных операций.
    ///
    /// # Аргументы
    ///
    /// * `check_fn` - Функция, которая возвращает ошибку, если операция должна быть прервана
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::{Connection, CheckSignalFn};
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let interrupted = Arc::new(AtomicBool::new(false));
    /// let interrupted_clone = interrupted.clone();
    ///
    /// let check_signal: CheckSignalFn = Arc::new(move || {
    ///     if interrupted_clone.load(Ordering::Relaxed) {
    ///         anyhow::bail!("Operation interrupted")
    ///     }
    ///     Ok(())
    /// });
    ///
    /// let conn = Connection::builder()
    ///     .check_signal_fn(check_signal)
    ///     .build()
    ///     .await?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn check_signal_fn(mut self, check_fn: CheckSignalFn) -> Self {
        self.check_signal_fn = Some(check_fn);
        self
    }

    /// Создает и устанавливает соединение с сервером CloudPub.
    ///
    /// Этот метод:
    /// 1. Проверяет конфигурацию
    /// 2. Инициализирует логирование
    /// 3. Загружает или создает файл конфигурации
    /// 4. Аутентифицируется на сервере (если предоставлены учетные данные)
    /// 5. Устанавливает соединение
    /// 6. Ожидает готовности соединения
    ///
    /// # Возвращает
    ///
    /// Возвращает экземпляр `Connection` при успехе, или ошибку, если:
    /// - Неверная конфигурация (например, email без пароля)
    /// - Неудачная аутентификация
    /// - Не удается установить соединение
    /// - Происходит таймаут
    ///
    /// # Пример
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use cloudpub_sdk::Connection;
    ///
    /// // Создание с настройками по умолчанию
    /// let conn = Connection::builder().build().await?;
    ///
    /// // Создание с пользовательской конфигурацией
    /// let conn = Connection::builder()
    ///     .credentials("user@example.com", "password")
    ///     .log_level("debug")
    ///     .verbose(true)
    ///     .timeout_secs(30)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ошибки
    ///
    /// Этот метод вернет ошибку, если:
    /// - Email предоставлен без пароля или наоборот
    /// - Не удалось инициализировать логирование
    /// - Не удалось загрузить или создать файл конфигурации
    /// - Неудачная аутентификация
    /// - Неудачное соединение с сервером
    /// - Таймаут ожидания соединения
    pub async fn build(self) -> Result<Connection> {
        // Проверяем аутентификацию, если она предоставлена
        if self.email.is_some() && self.password.is_none() {
            bail!("Password is required when email is provided");
        }
        if self.password.is_some() && self.email.is_none() {
            bail!("Email is required when password is provided");
        }

        // Увеличиваем лимит `nofile` на Linux и Mac
        if let Err(err) = fdlimit::raise_fd_limit() {
            warn!("Failed to raise file descriptor limit: {}", err);
        }

        // Создаем директорию для логов
        let log_dir = cache_dir().context("Can't get cache dir")?.join("cloudpub");
        std::fs::create_dir_all(&log_dir).context("Can't create log dir")?;

        let log_file = log_dir.join("client.log");

        // Инициализируем логирование
        let _guard = init_log(
            &self.log_level,
            &log_file,
            self.verbose,
            10 * 1024 * 1024,
            2,
        )
        .context("Failed to initialize logging")?;

        // Загружаем конфигурацию
        let mut config = if let Some(ref path) = self.config_path {
            ClientConfig::from_file(path, false)?
        } else {
            ClientConfig::load("client.toml", true, false)?
        };

        // Обрабатываем аутентификацию по токену
        if let Some(token_value) = self.token {
            config.token = Some(MaskedString(token_value));
        }

        let config = Arc::new(RwLock::new(config));

        // Настраиваем каналы
        let (command_tx, command_rx) = mpsc::channel(1024);
        let (result_tx, result_rx) = mpsc::channel(1024);

        // Определяем опции на основе аутентификации
        let opts = if let Some(email_value) = self.email {
            if let Some(password_value) = self.password {
                ClientOpts {
                    credentials: Some((email_value, password_value)),
                    ..Default::default()
                }
            } else {
                // Это не должно произойти из-за проверки выше
                ClientOpts::default()
            }
        } else {
            ClientOpts::default()
        };

        // Клонируем то, что нам нужно для задачи клиента
        let config_clone = config.clone();

        // Запускаем задачу клиента
        let client_handle = tokio::spawn(async move {
            if let Err(err) = run_client(config_clone, opts, command_rx, result_tx)
                .boxed()
                .await
            {
                tracing::error!("Client exited with error: {:?}, restarting in 5 sec..", err);
            }
        });

        // Создаем соединение со встроенным управлением событиями
        let connection = Connection::new(
            config,
            command_tx,
            result_rx,
            self.timeout,
            self.check_signal_fn,
            _guard,
            Some(client_handle),
        );

        // Ожидаем установления соединения
        connection
            .wait_for_event(|event| matches!(event, ConnectionEvent::Connected))
            .await?;

        Ok(connection)
    }
}
