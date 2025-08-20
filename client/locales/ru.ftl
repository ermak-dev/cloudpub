error-network = Ошибка сети, пробуем еще раз.
error-process-terminated = Процесс сервера был неожиданно завершен
error-auth-missing = Отсутствует токен авторизации
error-measurement = Ошибка измерения
error-finding-free-port = Ошибка поиска свободного порта

# Connection states
connecting = Подключение к серверу...
measuring-speed = Измеряем скорость подключения...

# Progress messages
downloading-webserver = Загрузка веб сервера
unpacking-webserver = Распаковка веб-сервера
downloading-vcpp = Загрузка компонентов VC++
installing-vcpp = Установка компонентов VC++

# Progress templates
progress-files = [{"{"}elapsed_precise{"}"}] {"{"}bar:40.cyan/blue{"}"} {"{"}pos{"}"}/{"{"}len{"}"} файлов
progress-files-eta = [{"{"}elapsed_precise{"}"}] {"{"}bar:40.cyan/blue{"}"} {"{"}pos{"}"}/{"{"}len{"}"} файлов ({"{"}eta{"}"})
progress-bytes = [{"{"}elapsed_precise{"}"}] {"{"}bar:40.cyan/blue{"}"} {"{"}pos{"}"}/{"{"}len{"}"} байт ({"{"}eta{"}"})

# Minecraft plugin messages
downloading-jdk = Загрузка JDK
installing-jdk = Установка JDK
downloading-minecraft-server = Загрузка сервера Minecraft
error-downloading-jdk = Ошибка загрузки JDK
error-unpacking-jdk = Ошибка распаковки JDK
error-copying-minecraft-server = Ошибка копирования сервера Minecraft: {$path}
error-invalid-minecraft-jar-directory = Неверный путь к JAR файлу сервера Minecraft: {$path} (директория)
error-downloading-minecraft-server = Ошибка загрузки сервера Minecraft: {$url}
error-invalid-minecraft-path = Неверный путь или URL к серверу Minecraft: {$path}
error-creating-server-directory = Ошибка создания директории сервера
error-creating-server-properties = Ошибка создания server.properties
error-creating-eula-file = Ошибка создания файла eula.txt
error-reading-server-properties = Ошибка чтения server.properties
error-writing-server-properties = Ошибка записи server.properties
error-getting-java-path = Ошибка получения пути к java

# Error contexts
error-downloading-webserver = Ошибка загрузки веб сервера
error-unpacking-webserver = Ошибка распаковки веб сервера
error-downloading-vcpp = Ошибка загрузки компонентов VC++
error-installing-vcpp = Ошибка установки компонентов VC++
error-setting-permissions = Ошибка установки прав на исполнение
error-creating-marker = Ошибка создания файла метки
error-writing-httpd-conf = Ошибка записи httpd.conf

# Service messages
service-published = Сервис опубликован: {$endpoint}
service-error = Ошибка публикации: {$endpoint}
service-registered = Сервис зарегистрирован: {$endpoint}
service-stopped = Сервис остановлен: {$guid}
service-removed = Сервис удален: {$guid}
no-registered-services = Нет зарегистрированных сервисов
all-services-removed = Все сервисы удалены

# Authentication
enter-email = Введите email:{" "}
enter-password = Введите пароль:{" "}
session-terminated = Сессия завершена, токен авторизации сброшен
client-authorized = Клиент успешно авторизован
upgrade-available = Доступна новая версия: {$version}. Выполните команду `clo upgrade` для обновления.

# Ping statistics
ping-time-percentiles = Время пинга (процентили):

# Invalid formats
invalid-url = Неверный URL
invalid-protocol = Неверный протокол
invalid-address = Неправильно указан адрес: {$address}
invalid-address-error = Неправильно указан адрес ({$error}): {$address}
port-required = Для этого прокола нужно указать порт

# Service messages
service-installed = Сервис успешно установлен
service-uninstalled = Сервис успешно удален
service-started = Сервис успешно запущен
service-stopped-service = Сервис успешно остановлен
service-running = Сервис запущен
service-stopped-status = Сервис остановлен
service-not-installed = Сервис не установлен
service-status-unknown = Статус сервиса неизвестен

# Update messages
applying-update = Применение обновления и перезапуск...
downloading-update = Загрузка обновления в {$path}
update-downloaded = Обновление загружено в: {$path}
update-unpacked = Обновление распаковано в: {$path}

# Cache messages
purge-cache-dir = Очистка кеш директории: {$path}

# GUI messages
show-window = Открыть CloudPub
quit = Завершить
